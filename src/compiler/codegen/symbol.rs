use crate::compiler::ast::{Expr, Param};
use crate::compiler::codegen::{CodeGen, StackItem, VariableState};
use crate::compiler::error::CompileError;
use inkwell::values::FunctionValue;
use inkwell::targets::{Target, TargetMachine, InitializationConfig, FileType};
use inkwell::OptimizationLevel;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

impl<'ctx> CodeGen<'ctx> {
    pub fn get_module_name(&self) -> String {
        use std::path::Path;
        Path::new(&self.input_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("main")
            .to_string()
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
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.input_path.hash(&mut hasher);
        let hash = hasher.finish();
        format!("__llm_{:x}_{}", hash, name)
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

    pub fn gen_function(&self, name: &str, params: Vec<Param>, body: &Expr, exported: bool) -> Result<FunctionValue<'ctx>, CompileError> {
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
        let function = self.module.add_function(&final_name, fn_type, None);
        if !exported && name != "main" {
            function.set_linkage(inkwell::module::Linkage::Internal);
        }
        
        let basic_block = self.context.append_basic_block(function, "entry");
        self.builder.position_at_end(basic_block);
        let mut stack = Vec::new();
        for i in 0..arg_count {
            stack.push(StackItem {
                value: function.get_nth_param(i as u32).unwrap(),
                state: VariableState::Available,
                shape: None,
                is_ptr: false, 
            });
        }
        let ret_val = self.gen_expr(body, &mut stack, &HashMap::new());
        for item in stack.iter() {
            if item.state == VariableState::Available {
                self.emit_auto_drop(item.value, item.shape.as_deref(), item.is_ptr);
            }
        }
        self.builder.build_return(Some(&ret_val)).unwrap();
        Ok(function)
    }

    pub fn emit_to_file(&self, path: &str) -> Result<(), String> {
        Target::initialize_all(&InitializationConfig::default());
        let target_triple = TargetMachine::get_default_triple();
        let target = Target::from_triple(&target_triple).map_err(|e| e.to_string())?;
        let target_machine = target
            .create_target_machine(
                &target_triple,
                "generic",
                "",
                OptimizationLevel::Default,
                inkwell::targets::RelocMode::Default,
                inkwell::targets::CodeModel::Default,
            )
            .ok_or_else(|| "Could not create target machine".to_string())?;

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
}
