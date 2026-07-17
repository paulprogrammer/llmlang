use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::builder::Builder;
use inkwell::values::{FunctionValue, BasicValueEnum, IntValue};
use crate::compiler::ast::{Expr, Param};
use std::collections::HashMap;
use crate::Config;
use std::hash::{Hash, Hasher};

pub mod expr;
pub mod symbol;
pub mod shape;
pub mod parallel;
pub mod soa;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VariableState {
    Available,
    Moved,
    Borrowed,
    MutBorrowed,
}

pub struct StackItem<'ctx> {
    pub value: BasicValueEnum<'ctx>,
    pub state: VariableState,
    pub shape: Option<String>,
    pub is_ptr: bool,
}

fn get_stack_info() -> (usize, usize) {
    #[cfg(target_os = "macos")]
    unsafe {
        let thread = libc::pthread_self();
        let stackaddr = libc::pthread_get_stackaddr_np(thread) as usize;
        let stacksize = libc::pthread_get_stacksize_np(thread);
        let stack_bottom = stackaddr - stacksize;
        return (stack_bottom, stacksize);
    }
    #[cfg(target_os = "linux")]
    unsafe {
        let mut attr: libc::pthread_attr_t = std::mem::zeroed();
        let mut stackaddr: *mut libc::c_void = std::ptr::null_mut();
        let mut stacksize: libc::size_t = 0;
        if libc::pthread_getattr_np(libc::pthread_self(), &mut attr) == 0 {
            let res = libc::pthread_attr_getstack(&attr, &mut stackaddr, &mut stacksize);
            libc::pthread_attr_destroy(&mut attr);
            if res == 0 {
                return (stackaddr as usize, stacksize);
            }
        }
    }
    (0, 8 * 1024 * 1024)
}

pub struct CodeGen<'ctx> {
    pub context: &'ctx Context,
    pub module: Module<'ctx>,
    pub builder: Builder<'ctx>,
    pub shapes: std::cell::RefCell<HashMap<String, Vec<String>>>,
    pub warnings: std::cell::RefCell<Vec<String>>,
    pub templates: std::cell::RefCell<HashMap<String, (Vec<Param>, Expr)>>,
    pub config: Config,
    pub input_path: String,
    /// File stem of input_path, computed once (input_path never changes after construction).
    pub module_name: String,
    /// `__llm_{hash(input_path):x}_` prefix used by mangle_name, computed once.
    pub mangle_prefix: String,
    pub has_exports: std::cell::Cell<bool>,
    pub exports: std::cell::RefCell<Vec<String>>,
    pub imports: std::cell::RefCell<HashMap<String, String>>,
    pub fn_returns_ptr: std::cell::RefCell<HashMap<String, bool>>,
    pub fn_param_ptrs: std::cell::RefCell<HashMap<String, Vec<bool>>>,
    pub parallel_depth: std::cell::Cell<usize>,
    pub max_parallel_depth: usize,
    pub stack_bottom: usize,
    pub stack_size: usize,
    /// Monotonic counter for naming synthesized trap/parallel functions.
    pub synth_id: std::cell::Cell<usize>,
}

impl<'ctx> CodeGen<'ctx> {
    /// Unique ID for synthesized helper functions (trap try/fallback,
    /// parallel tasks). Shared across both so names never collide.
    pub fn next_synth_id(&self) -> usize {
        let id = self.synth_id.get();
        self.synth_id.set(id + 1);
        id
    }

    pub fn has_sufficient_stack(&self) -> bool {
        if self.stack_bottom == 0 {
            return true;
        }
        let local_var = 0;
        let current_sp = &local_var as *const _ as usize;
        // Require at least 256 KB of remaining stack space to continue hoisting
        let min_safe_remaining = 256 * 1024;
        current_sp > self.stack_bottom + min_safe_remaining
    }

    pub fn new(context: &'ctx Context, module_name: &str, config: Config) -> Self {
        let module = context.create_module(module_name);
        let builder = context.create_builder();
        
        // Emit runtime configuration as global constants with WeakODR linkage
        let i64_type = context.i64_type();
        let threads_global = module.add_global(i64_type, None, "llm_max_threads");
        threads_global.set_initializer(&i64_type.const_int(config.max_threads as u64, false));
        threads_global.set_constant(true);
        threads_global.set_linkage(inkwell::module::Linkage::WeakODR);

        let queue_global = module.add_global(i64_type, None, "llm_queue_size");
        queue_global.set_initializer(&i64_type.const_int(config.queue_size as u64, false));
        queue_global.set_constant(true);
        queue_global.set_linkage(inkwell::module::Linkage::WeakODR);

        // Prepopulate with built-in/FFI functions that return pointers
        let mut fn_returns_ptr = HashMap::new();
        let ffi_funcs = vec![
            "http_get", "http_post", "get", "post",
            "json_parse", "parse",
            "json_stringify", "stringify",
            "json_get_str", "get_str",
            "sign", "encrypt", "decrypt", "unwrap",
            "serve", "https_serve", "accept",
            "connect", "connect_binding", "query", "error",
            "db_connect", "db_connect_binding", "db_query", "db_error"
        ];
        for f in ffi_funcs {
            fn_returns_ptr.insert(f.to_string(), true);
        }

        let (stack_bottom, stack_size) = get_stack_info();
        let max_parallel_depth = (stack_size / 32768).max(1);

        let cached_module_name = std::path::Path::new(module_name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("main")
            .to_string();
        let mangle_prefix = {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            module_name.hash(&mut hasher);
            format!("__llm_{:x}_", hasher.finish())
        };

        Self {
            context, 
            module, 
            builder,
            shapes: std::cell::RefCell::new(HashMap::new()),
            warnings: std::cell::RefCell::new(Vec::new()),
            templates: std::cell::RefCell::new(HashMap::new()),
            config,
            input_path: module_name.to_string(),
            module_name: cached_module_name,
            mangle_prefix,
            has_exports: std::cell::Cell::new(false),
            exports: std::cell::RefCell::new(Vec::new()),
            imports: std::cell::RefCell::new(HashMap::new()),
            fn_returns_ptr: std::cell::RefCell::new(fn_returns_ptr),
            fn_param_ptrs: std::cell::RefCell::new(HashMap::new()),
            parallel_depth: std::cell::Cell::new(0),
            max_parallel_depth,
            stack_bottom,
            stack_size,
            synth_id: std::cell::Cell::new(0),
        }
    }

    fn get_llvm_type(&self, name: &str) -> inkwell::types::BasicTypeEnum<'ctx> {
        match name {
            "i64" => self.context.i64_type().into(),
            "f64" => self.context.f64_type().into(),
            _ => self.context.i64_type().into(),
        }
    }

    fn get_or_add_external_fn(&self, name: &str, fn_type: inkwell::types::FunctionType<'ctx>) -> FunctionValue<'ctx> {
        if let Some(f) = self.module.get_function(name) {
            f
        } else {
            self.module.add_function(name, fn_type, None)
        }
    }

    fn get_call_res(&self, call: inkwell::values::CallSiteValue<'ctx>) -> BasicValueEnum<'ctx> {
        use inkwell::values::AsValueRef;
        unsafe { BasicValueEnum::new(call.as_value_ref()) }
    }

    fn as_int(&self, val: BasicValueEnum<'ctx>) -> IntValue<'ctx> {
        if val.is_int_value() {
            val.into_int_value()
        } else if val.is_pointer_value() {
            self.builder.build_ptr_to_int(val.into_pointer_value(), self.context.i64_type(), "ptr_to_int").unwrap()
        } else if val.is_float_value() {
            self.builder.build_float_to_signed_int(val.into_float_value(), self.context.i64_type(), "f_to_i").unwrap()
        } else {
            panic!("E996: Expected integer-convertible value");
        }
    }

    fn gen_string_constant(&self, s: &str) -> BasicValueEnum<'ctx> {
        let string_val = self.context.const_string(s.as_bytes(), true);
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        s.hash(&mut hasher);
        let hash = hasher.finish();
        let global_name = format!("str_const_{:x}", hash);

        // Must mirror LlmRtHeader in src/runtime/common.h: {magic: u32,
        // type: u32, ref_cnt: atomic u32} — three 4-byte fields, no padding.
        let header_type = self.context.struct_type(&[
            self.context.i32_type().into(),
            self.context.i32_type().into(),
            self.context.i32_type().into(),
        ], false);

        let struct_type = self.context.struct_type(&[
            header_type.into(),
            string_val.get_type().into(),
        ], false);

        let global = if let Some(g) = self.module.get_global(&global_name) {
            g
        } else {
            let magic_val = self.context.i32_type().const_int(0, false);
            let type_val = self.context.i32_type().const_int(1, false); // RT_TYPE_STRING = 1
            let ref_cnt_val = self.context.i32_type().const_int(1, false);

            let header_val = header_type.const_named_struct(&[
                magic_val.into(),
                type_val.into(),
                ref_cnt_val.into(),
            ]);

            let global_val = struct_type.const_named_struct(&[
                header_val.into(),
                string_val.into(),
            ]);

            let g = self.module.add_global(struct_type, None, &global_name);
            g.set_initializer(&global_val);
            g.set_constant(true);
            g.set_linkage(inkwell::module::Linkage::LinkOnceODR);
            g
        };

        let ptr = global.as_pointer_value();
        let str_ptr = self.builder.build_struct_gep(struct_type, ptr, 1, "str_ptr").unwrap();
        let ptr_int = self.builder.build_ptr_to_int(str_ptr, self.context.i64_type(), "str_ptr_int").unwrap();
        ptr_int.into()
    }

    fn emit_auto_drop(&self, val: BasicValueEnum<'ctx>, shape: Option<&str>, is_ptr: bool) {
        if !is_ptr { return; }
        if let Some(s) = shape {
            let shapes = self.shapes.borrow();
            let fields = shapes.get(s).expect("E006");
            let field_count = fields.len() as u64;
            let fn_type = self.context.void_type().fn_type(&[self.context.i64_type().into(), self.context.i64_type().into()], false);
            let func = self.get_or_add_external_fn("llm_drop_soa", fn_type);
            self.builder.build_call(func, &[val.into(), self.context.i64_type().const_int(field_count, false).into()], "").unwrap();
        } else {
            let fn_type = self.context.void_type().fn_type(&[self.context.i64_type().into()], false);
            let func = self.get_or_add_external_fn("llm_drop", fn_type);
            self.builder.build_call(func, &[val.into()], "").unwrap();
        }
    }
}
