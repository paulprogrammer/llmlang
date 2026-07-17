use crate::compiler::ast::{Expr, Param};
use crate::compiler::codegen::{CodeGen, StackItem, VariableState};
use crate::compiler::error::CompileError;
use inkwell::values::FunctionValue;
use inkwell::targets::{Target, TargetMachine, InitializationConfig, FileType};
use inkwell::OptimizationLevel;
use std::collections::HashMap;

use inkwell::passes::PassBuilderOptions;

impl<'ctx> CodeGen<'ctx> {
    pub fn get_module_name(&self) -> &str {
        // Computed once in CodeGen::new; input_path never changes afterwards.
        &self.module_name
    }

    pub fn resolve_func_name(&self, name: &str) -> String {
        if let Some(module) = self.imports.borrow().get(name) {
            if module == "main" || module == "test" {
                name.to_string()
            } else {
                format!("{}_{}", module, name)
            }
        } else {
            let mod_name = self.get_module_name();
            let namespaced = if mod_name == "main" || mod_name == "test" {
                name.to_string()
            } else {
                format!("{}_{}", mod_name, name)
            };
            let local_mangled = self.mangle_name(name);
            
            if self.module.get_function(&namespaced).is_some() {
                namespaced
            } else if self.module.get_function(&local_mangled).is_some() {
                local_mangled
            } else if self.module.get_function(name).is_some() {
                name.to_string()
            } else {
                local_mangled
            }
        }
    }

    pub fn mangle_name(&self, name: &str) -> String {
        if name == "main" { return name.to_string(); }
        // mangle_prefix is the input_path hash, computed once in CodeGen::new.
        format!("{}{}", self.mangle_prefix, name)
    }

    pub fn gen_import(&self, _module_alias: &str, symbol_name: &str, arity: usize) {
        self.imports.borrow_mut().insert(symbol_name.to_string(), _module_alias.to_string());
        let i64_type = self.context.i64_type();
        let args_types = vec![i64_type.into(); arity];
        let fn_type = i64_type.fn_type(&args_types, false);
        let mangled_name = if _module_alias == "main" || _module_alias == "test" {
            symbol_name.to_string()
        } else {
            format!("{}_{}", _module_alias, symbol_name)
        };
        self.module.add_function(&mangled_name, fn_type, None);
    }

    pub fn gen_function(&self, name: &str, params: Vec<Param>, body: &Expr, exported: bool, otel_span_name: Option<String>) -> Result<FunctionValue<'ctx>, CompileError> {
        // Run semantic verification
        {
            let mut shapes = HashMap::new();
            for (shape_name, fields) in self.shapes.borrow().iter() {
                shapes.insert(shape_name.clone(), fields.clone());
            }

            let mut functions = HashMap::new();
            for call_name in body.get_calls() {
                let resolved = self.resolve_func_name(&call_name);
                if self.module.get_function(&resolved).is_some() 
                    || self.templates.borrow().contains_key(&resolved) 
                    || self.module.get_function(&call_name).is_some() 
                    || self.fn_param_ptrs.borrow().contains_key(&resolved)
                    || self.fn_param_ptrs.borrow().contains_key(&call_name)
                {
                    functions.insert(call_name, 0);
                }
            }
            functions.insert(name.to_string(), params.len());

            let mut verify_stack = Vec::new();
            let mut verify_shapes = Vec::new();
            let mut verify_expand = HashMap::new();
            for (i, param) in params.iter().enumerate() {
                verify_stack.push(VariableState::Available);
                verify_shapes.push(None);
                if param.expand {
                    verify_expand.insert(param.name.clone(), i);
                }
            }
            let mut verify_ctx = crate::compiler::analysis::verify::VerificationContext {
                shapes,
                functions,
                stack: verify_stack,
                stack_shapes: verify_shapes,
                expand_map: verify_expand,
            };
            if let Err(err_code) = crate::compiler::analysis::verify::verify_expr(body, &mut verify_ctx) {
                return Err(CompileError::new(&err_code, &self.input_path, 1));
            }
        }

        if exported { 
            self.has_exports.set(true); 
            self.exports.borrow_mut().push(name.to_string());
        }
        let final_name = if name == "main" {
            name.to_string()
        } else if exported {
            let mod_name = self.get_module_name();
            if mod_name == "main" || mod_name == "test" {
                name.to_string()
            } else {
                format!("{}_{}", mod_name, name)
            }
        } else {
            self.mangle_name(name)
        };

        if params.iter().any(|p| p.expand) {
            self.templates.borrow_mut().insert(final_name.clone(), (params, body.clone()));
            let fn_type = self.context.i64_type().fn_type(&[], false);
            return Ok(self.module.add_function(&final_name, fn_type, None));
        }

        let arg_count = params.len();
        let i64_type = self.context.i64_type();
        let args_types = vec![i64_type.into(); arg_count];
        let fn_type = i64_type.fn_type(&args_types, false);
        let function = if let Some(f) = self.module.get_function(&final_name) {
            f
        } else {
            self.module.add_function(&final_name, fn_type, None)
        };
        if !exported && name != "main" {
            function.set_linkage(inkwell::module::Linkage::Internal);
        }
        
        let basic_block = self.context.append_basic_block(function, "entry");
        self.builder.position_at_end(basic_block);
        let mut stack = Vec::new();
        let param_ptrs = self.fn_param_ptrs.borrow().get(name).cloned().unwrap_or_else(|| vec![false; arg_count]);
        for i in 0..arg_count {
            let is_ptr = param_ptrs.get(i).copied().unwrap_or(false);
            stack.push(StackItem {
                value: function.get_nth_param(i as u32).unwrap(),
                state: VariableState::Available,
                shape: None,
                is_ptr, 
            });
        }
        let mut start_time = None;
        if otel_span_name.is_some() {
            let get_time_fn = self.get_or_add_external_fn("llm_get_time_ns", self.context.i64_type().fn_type(&[], false));
            start_time = Some(self.get_call_res(self.builder.build_call(get_time_fn, &[], "start_time").unwrap()));
            
            let enter_span_fn = self.get_or_add_external_fn("llm_otel_enter_span", self.context.void_type().fn_type(&[], false));
            self.builder.build_call(enter_span_fn, &[], "enter_span").unwrap();
        }
        
        let ret_val = self.gen_expr(body, &mut stack, &HashMap::new());
        
        if let (Some(span_name), Some(start)) = (otel_span_name, start_time) {
            let get_time_fn = self.get_or_add_external_fn("llm_get_time_ns", self.context.i64_type().fn_type(&[], false));
            let end_time = self.get_call_res(self.builder.build_call(get_time_fn, &[], "end_time").unwrap());
            
            let emit_span_fn = self.get_or_add_external_fn("llm_otel_emit_span", self.context.void_type().fn_type(&[
                self.context.i64_type().into(), // name (string ptr as i64)
                self.context.i64_type().into(), // start
                self.context.i64_type().into()  // end
            ], false));
            
            let name_ptr = self.gen_string_constant(&span_name);
            self.builder.build_call(emit_span_fn, &[
                self.as_int(name_ptr).into(),
                start.into(),
                end_time.into()
            ], "emit_span").unwrap();
            
            let exit_span_fn = self.get_or_add_external_fn("llm_otel_exit_span", self.context.void_type().fn_type(&[], false));
            self.builder.build_call(exit_span_fn, &[], "exit_span").unwrap();
        }
        for item in stack.iter() {
            if item.state == VariableState::Available {
                self.emit_auto_drop(item.value, item.shape.as_deref(), item.is_ptr);
            }
        }
        
        if final_name == "main" {
            let wait_all_fn = if let Some(f) = self.module.get_function("llm_emit_wait_all") {
                f
            } else {
                self.module.add_function("llm_emit_wait_all", self.context.void_type().fn_type(&[], false), None)
            };
            self.builder.build_call(wait_all_fn, &[], "wait_all").unwrap();
        }
        
        self.builder.build_return(Some(&ret_val)).unwrap();
        Ok(function)
    }

    pub fn emit_to_file(&self, path: &str) -> Result<(), String> {
        Target::initialize_all(&InitializationConfig::default());
        let target_triple = TargetMachine::get_default_triple();
        let target = Target::from_triple(&target_triple).map_err(|e| e.to_string())?;
        
        let host_cpu = TargetMachine::get_host_cpu_name();
        let host_features = TargetMachine::get_host_cpu_features();
        let cpu_str = host_cpu.to_str().unwrap_or("generic");
        let features_str = host_features.to_str().unwrap_or("");

        let target_machine = target
            .create_target_machine(
                &target_triple,
                cpu_str,
                features_str,
                OptimizationLevel::Aggressive,
                inkwell::targets::RelocMode::Default,
                inkwell::targets::CodeModel::Default,
            )
            .ok_or_else(|| "Could not create target machine".to_string())?;

        // Run LLVM vectorization passes via PassBuilderOptions
        let options = PassBuilderOptions::create();
        options.set_loop_vectorization(true);
        options.set_loop_slp_vectorization(true);
        self.module
            .run_passes("default<O3>", &target_machine, options)
            .map_err(|e| e.to_string())?;

        target_machine
            .write_to_file(&self.module, FileType::Object, path.as_ref())
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    pub fn emit_signature_file(&self) -> String {
        let mut sig = String::new();
        let exports = self.exports.borrow();
        let shapes = self.shapes.borrow();
        
        let mod_name = self.get_module_name();
        let prefix = if mod_name == "main" || mod_name == "test" {
            String::new()
        } else {
            format!("{}_", mod_name)
        };

        for (name, fields) in shapes.iter() {
            let clean_name = if !prefix.is_empty() && name.starts_with(&prefix) {
                &name[prefix.len()..]
            } else {
                name
            };
            if exports.contains(&clean_name.to_string()) {
                sig.push_str(&format!("# {} {}\n", clean_name, fields.join(" ")));
            }
        }
        for func in self.module.get_functions() {
            let full_name = func.get_name().to_str().unwrap();
            if full_name == "main" || full_name.starts_with("trap_") || full_name.starts_with("parallel_") || full_name.starts_with("__llm_") { continue; }
            let name = if !prefix.is_empty() && full_name.starts_with(&prefix) {
                &full_name[prefix.len()..]
            } else {
                full_name
            };
            if exports.contains(&name.to_string()) {
                let param_count = func.count_params();
                sig.push_str(&format!(": {} {}\n", name, param_count));
            }
        }
        for (name, (params, _)) in self.templates.borrow().iter() {
            let clean_name = if !prefix.is_empty() && name.starts_with(&prefix) {
                &name[prefix.len()..]
            } else {
                name
            };
            if exports.contains(&clean_name.to_string()) {
                sig.push_str(&format!(": {} {}\n", clean_name, params.len()));
            }
        }
        sig
    }

    pub fn analyze_module_types(&self, exprs: &[Expr]) {
        let mut fn_returns_ptr = self.fn_returns_ptr.borrow_mut();
        let mut fn_param_ptrs = self.fn_param_ptrs.borrow_mut();
        
        // 1. Initialize user-defined functions
        for expr in exprs {
            let actual_expr = match expr {
                Expr::Metadata(_, _, t) => &**t,
                _ => expr,
            };
            if let Expr::Define(name, params, _, _) = actual_expr {
                fn_returns_ptr.insert(name.clone(), false);
                let mangled = self.mangle_name(name);
                let resolved = self.resolve_func_name(name);
                fn_returns_ptr.insert(mangled.clone(), false);
                fn_returns_ptr.insert(resolved.clone(), false);
                
                fn_param_ptrs.insert(name.clone(), vec![false; params.len()]);
                fn_param_ptrs.insert(mangled.clone(), vec![false; params.len()]);
                fn_param_ptrs.insert(resolved.clone(), vec![false; params.len()]);
            }
        }
        
        // 2. Iterate to fixed point
        let mut changed = true;
        let mut iterations = 0;
        while changed && iterations < 100 {
            changed = false;
            iterations += 1;
            
            for expr in exprs {
                let actual_expr = match expr {
                    Expr::Metadata(_, _, t) => &**t,
                    _ => expr,
                };
                if let Expr::Define(name, params, body, _) = actual_expr {
                    let mangled = self.mangle_name(name);
                    let resolved = self.resolve_func_name(name);
                    
                    let current_params = fn_param_ptrs.get(name).cloned().unwrap_or_else(|| vec![false; params.len()]);
                    
                    // Run a recursive constraints check on body
                    let mut stack_ptrs = current_params.clone();
                    let mut new_params = current_params.clone();
                    
                    self.infer_constraints(body, &mut stack_ptrs, &mut new_params, &fn_returns_ptr, &fn_param_ptrs);
                    
                    if new_params != current_params {
                        fn_param_ptrs.insert(name.clone(), new_params.clone());
                        fn_param_ptrs.insert(mangled.clone(), new_params.clone());
                        fn_param_ptrs.insert(resolved.clone(), new_params.clone());
                        changed = true;
                    }
                    
                    // Determine if the body returns a pointer
                    let returns_ptr = body.returns_ptr_with_stack(&fn_param_ptrs.get(name).unwrap(), &fn_returns_ptr);
                    if returns_ptr {
                        if fn_returns_ptr.get(name) != Some(&true) {
                            fn_returns_ptr.insert(name.clone(), true);
                            fn_returns_ptr.insert(mangled.clone(), true);
                            fn_returns_ptr.insert(resolved.clone(), true);
                            changed = true;
                        }
                    }
                }
            }
        }
    }

    fn infer_constraints(
        &self,
        expr: &Expr,
        stack_ptrs: &mut Vec<bool>,
        param_ptrs: &mut Vec<bool>,
        fn_returns_ptr: &HashMap<String, bool>,
        fn_param_ptrs: &HashMap<String, Vec<bool>>,
    ) {
        match expr {
            Expr::Let(_, val_expr, body_expr) => {
                self.infer_constraints(val_expr, stack_ptrs, param_ptrs, fn_returns_ptr, fn_param_ptrs);
                let val_ptr = val_expr.returns_ptr_with_stack(stack_ptrs, fn_returns_ptr);
                stack_ptrs.push(val_ptr);
                self.infer_constraints(body_expr, stack_ptrs, param_ptrs, fn_returns_ptr, fn_param_ptrs);
                stack_ptrs.pop();
            }
            Expr::Seq(l, r) => {
                self.infer_constraints(l, stack_ptrs, param_ptrs, fn_returns_ptr, fn_param_ptrs);
                self.infer_constraints(r, stack_ptrs, param_ptrs, fn_returns_ptr, fn_param_ptrs);
            }
            Expr::If(c, t, f) => {
                self.infer_constraints(c, stack_ptrs, param_ptrs, fn_returns_ptr, fn_param_ptrs);
                self.infer_constraints(t, stack_ptrs, param_ptrs, fn_returns_ptr, fn_param_ptrs);
                self.infer_constraints(f, stack_ptrs, param_ptrs, fn_returns_ptr, fn_param_ptrs);
            }
            Expr::Move(e) | Expr::Borrow(e) | Expr::MutBorrow(e) => {
                self.infer_constraints(e, stack_ptrs, param_ptrs, fn_returns_ptr, fn_param_ptrs);
            }
            Expr::Trap(t, f) => {
                self.infer_constraints(t, stack_ptrs, param_ptrs, fn_returns_ptr, fn_param_ptrs);
                self.infer_constraints(f, stack_ptrs, param_ptrs, fn_returns_ptr, fn_param_ptrs);
            }
            Expr::Cat(l, r) => {
                self.mark_as_ptr(l, stack_ptrs, param_ptrs);
                self.mark_as_ptr(r, stack_ptrs, param_ptrs);
                self.infer_constraints(l, stack_ptrs, param_ptrs, fn_returns_ptr, fn_param_ptrs);
                self.infer_constraints(r, stack_ptrs, param_ptrs, fn_returns_ptr, fn_param_ptrs);
            }
            Expr::Sub(s, b, l) => {
                self.mark_as_ptr(s, stack_ptrs, param_ptrs);
                self.infer_constraints(s, stack_ptrs, param_ptrs, fn_returns_ptr, fn_param_ptrs);
                self.infer_constraints(b, stack_ptrs, param_ptrs, fn_returns_ptr, fn_param_ptrs);
                self.infer_constraints(l, stack_ptrs, param_ptrs, fn_returns_ptr, fn_param_ptrs);
            }
            Expr::Len(e) => {
                self.mark_as_ptr(e, stack_ptrs, param_ptrs);
                self.infer_constraints(e, stack_ptrs, param_ptrs, fn_returns_ptr, fn_param_ptrs);
            }
            Expr::Loc(s, p) => {
                self.mark_as_ptr(s, stack_ptrs, param_ptrs);
                self.mark_as_ptr(p, stack_ptrs, param_ptrs);
                self.infer_constraints(s, stack_ptrs, param_ptrs, fn_returns_ptr, fn_param_ptrs);
                self.infer_constraints(p, stack_ptrs, param_ptrs, fn_returns_ptr, fn_param_ptrs);
            }
            Expr::Reg(s, r) => {
                self.mark_as_ptr(s, stack_ptrs, param_ptrs);
                self.mark_as_ptr(r, stack_ptrs, param_ptrs);
                self.infer_constraints(s, stack_ptrs, param_ptrs, fn_returns_ptr, fn_param_ptrs);
                self.infer_constraints(r, stack_ptrs, param_ptrs, fn_returns_ptr, fn_param_ptrs);
            }
            Expr::Split(s, d, idx) => {
                self.mark_as_ptr(s, stack_ptrs, param_ptrs);
                self.mark_as_ptr(d, stack_ptrs, param_ptrs);
                self.infer_constraints(s, stack_ptrs, param_ptrs, fn_returns_ptr, fn_param_ptrs);
                self.infer_constraints(d, stack_ptrs, param_ptrs, fn_returns_ptr, fn_param_ptrs);
                self.infer_constraints(idx, stack_ptrs, param_ptrs, fn_returns_ptr, fn_param_ptrs);
            }
            Expr::Write(h, s) => {
                self.mark_as_ptr(h, stack_ptrs, param_ptrs);
                self.mark_as_ptr(s, stack_ptrs, param_ptrs);
                self.infer_constraints(h, stack_ptrs, param_ptrs, fn_returns_ptr, fn_param_ptrs);
                self.infer_constraints(s, stack_ptrs, param_ptrs, fn_returns_ptr, fn_param_ptrs);
            }
            Expr::Apply(f, args) => {
                self.infer_constraints(f, stack_ptrs, param_ptrs, fn_returns_ptr, fn_param_ptrs);
                for arg in args {
                    self.infer_constraints(arg, stack_ptrs, param_ptrs, fn_returns_ptr, fn_param_ptrs);
                }
                if let Expr::Identifier(ref callee_name) = **f {
                    let resolved = self.resolve_func_name(callee_name);
                    
                    if resolved == "verify" || resolved == "crypto_verify" {
                        if args.len() >= 3 {
                            self.mark_as_ptr(&args[0], stack_ptrs, param_ptrs);
                            self.mark_as_ptr(&args[1], stack_ptrs, param_ptrs);
                            self.mark_as_ptr(&args[2], stack_ptrs, param_ptrs);
                        }
                    } else if resolved == "sign" || resolved == "crypto_sign" {
                        if args.len() >= 2 {
                            self.mark_as_ptr(&args[0], stack_ptrs, param_ptrs);
                            self.mark_as_ptr(&args[1], stack_ptrs, param_ptrs);
                        }
                    } else if let Some(param_types) = fn_param_ptrs.get(&resolved).or_else(|| fn_param_ptrs.get(callee_name)) {
                        for (arg, &is_ptr) in args.iter().zip(param_types.iter()) {
                            if is_ptr {
                                self.mark_as_ptr(arg, stack_ptrs, param_ptrs);
                            }
                        }
                    }
                }
            }
            Expr::Get(inst, _, idx) => {
                self.mark_as_ptr(inst, stack_ptrs, param_ptrs);
                self.infer_constraints(inst, stack_ptrs, param_ptrs, fn_returns_ptr, fn_param_ptrs);
                self.infer_constraints(idx, stack_ptrs, param_ptrs, fn_returns_ptr, fn_param_ptrs);
            }
            Expr::Set(inst, _, idx, val) => {
                self.mark_as_ptr(inst, stack_ptrs, param_ptrs);
                self.infer_constraints(inst, stack_ptrs, param_ptrs, fn_returns_ptr, fn_param_ptrs);
                self.infer_constraints(idx, stack_ptrs, param_ptrs, fn_returns_ptr, fn_param_ptrs);
                self.infer_constraints(val, stack_ptrs, param_ptrs, fn_returns_ptr, fn_param_ptrs);
            }
            Expr::Unpack(expr, _) => {
                self.mark_as_ptr(expr, stack_ptrs, param_ptrs);
                self.infer_constraints(expr, stack_ptrs, param_ptrs, fn_returns_ptr, fn_param_ptrs);
            }
            Expr::Pack(expr) => {
                self.mark_as_ptr(expr, stack_ptrs, param_ptrs);
                self.infer_constraints(expr, stack_ptrs, param_ptrs, fn_returns_ptr, fn_param_ptrs);
            }
            Expr::Map(inst, _, func) => {
                self.mark_as_ptr(inst, stack_ptrs, param_ptrs);
                self.infer_constraints(inst, stack_ptrs, param_ptrs, fn_returns_ptr, fn_param_ptrs);
                self.infer_constraints(func, stack_ptrs, param_ptrs, fn_returns_ptr, fn_param_ptrs);
            }
            Expr::Filter(inst, func) => {
                self.mark_as_ptr(inst, stack_ptrs, param_ptrs);
                self.infer_constraints(inst, stack_ptrs, param_ptrs, fn_returns_ptr, fn_param_ptrs);
                self.infer_constraints(func, stack_ptrs, param_ptrs, fn_returns_ptr, fn_param_ptrs);
            }
            Expr::HttpClient(m, u, b) => {
                self.mark_as_ptr(m, stack_ptrs, param_ptrs);
                self.mark_as_ptr(u, stack_ptrs, param_ptrs);
                self.mark_as_ptr(b, stack_ptrs, param_ptrs);
                self.infer_constraints(m, stack_ptrs, param_ptrs, fn_returns_ptr, fn_param_ptrs);
                self.infer_constraints(u, stack_ptrs, param_ptrs, fn_returns_ptr, fn_param_ptrs);
                self.infer_constraints(b, stack_ptrs, param_ptrs, fn_returns_ptr, fn_param_ptrs);
            }
            Expr::HttpServer(_, arg) => {
                self.mark_as_ptr(arg, stack_ptrs, param_ptrs);
                self.infer_constraints(arg, stack_ptrs, param_ptrs, fn_returns_ptr, fn_param_ptrs);
            }
            Expr::FileOpen(p, m) => {
                self.mark_as_ptr(p, stack_ptrs, param_ptrs);
                self.mark_as_ptr(m, stack_ptrs, param_ptrs);
                self.infer_constraints(p, stack_ptrs, param_ptrs, fn_returns_ptr, fn_param_ptrs);
                self.infer_constraints(m, stack_ptrs, param_ptrs, fn_returns_ptr, fn_param_ptrs);
            }
            _ => {}
        }
    }

    fn mark_as_ptr(&self, expr: &Expr, stack_ptrs: &mut [bool], param_ptrs: &mut [bool]) {
        match expr {
            Expr::DeBruijn(idx) => {
                if *idx < stack_ptrs.len() {
                    let actual_idx = stack_ptrs.len() - 1 - idx;
                    stack_ptrs[actual_idx] = true;
                    if actual_idx < param_ptrs.len() {
                        param_ptrs[actual_idx] = true;
                    }
                }
            }
            Expr::Move(e) | Expr::Borrow(e) | Expr::MutBorrow(e) => {
                self.mark_as_ptr(e, stack_ptrs, param_ptrs);
            }
            _ => {}
        }
    }
}
