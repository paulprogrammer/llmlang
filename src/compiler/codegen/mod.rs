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

pub struct CodeGen<'ctx> {
    pub context: &'ctx Context,
    pub module: Module<'ctx>,
    pub builder: Builder<'ctx>,
    pub shapes: std::cell::RefCell<HashMap<String, Vec<String>>>,
    pub warnings: std::cell::RefCell<Vec<String>>,
    pub templates: std::cell::RefCell<HashMap<String, (Vec<Param>, Expr)>>,
    pub config: Config,
    pub input_path: String,
    pub has_exports: std::cell::Cell<bool>,
    pub exports: std::cell::RefCell<Vec<String>>,
    pub imports: std::cell::RefCell<HashMap<String, String>>,
}

impl<'ctx> CodeGen<'ctx> {
    pub fn new(context: &'ctx Context, module_name: &str, config: Config) -> Self {
        let module = context.create_module(module_name);
        let builder = context.create_builder();
        
        // Emit runtime configuration as global constants with LinkOnceODR linkage
        let i64_type = context.i64_type();
        let threads_global = module.add_global(i64_type, None, "llm_max_threads");
        threads_global.set_initializer(&i64_type.const_int(config.max_threads as u64, false));
        threads_global.set_constant(true);
        threads_global.set_linkage(inkwell::module::Linkage::LinkOnceODR);

        let queue_global = module.add_global(i64_type, None, "llm_queue_size");
        queue_global.set_initializer(&i64_type.const_int(config.queue_size as u64, false));
        queue_global.set_constant(true);
        queue_global.set_linkage(inkwell::module::Linkage::LinkOnceODR);

        Self { 
            context, 
            module, 
            builder,
            shapes: std::cell::RefCell::new(HashMap::new()),
            warnings: std::cell::RefCell::new(Vec::new()),
            templates: std::cell::RefCell::new(HashMap::new()),
            config,
            input_path: module_name.to_string(),
            has_exports: std::cell::Cell::new(false),
            exports: std::cell::RefCell::new(Vec::new()),
            imports: std::cell::RefCell::new(HashMap::new()),
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

        let global = if let Some(g) = self.module.get_global(&global_name) {
            g
        } else {
            let g = self.module.add_global(string_val.get_type(), None, &global_name);
            g.set_initializer(&string_val);
            g.set_constant(true);
            g.set_linkage(inkwell::module::Linkage::LinkOnceODR);
            g
        };

        let ptr = global.as_pointer_value();
        let ptr_int = self.builder.build_ptr_to_int(ptr, self.context.i64_type(), "str_ptr").unwrap();

        let fn_type = self.context.i64_type().fn_type(&[self.context.i64_type().into()], false);
        let func = self.get_or_add_external_fn("llm_strdup", fn_type);
        let call = self.builder.build_call(func, &[ptr_int.into()], "str_heap").unwrap();
        self.get_call_res(call)
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
