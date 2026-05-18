use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::builder::Builder;
use inkwell::values::{FunctionValue, BasicValueEnum};
use crate::parser::{Expr, Param};
use crate::lexer::Token;
use std::collections::HashMap;
use inkwell::targets::{Target, TargetMachine, InitializationConfig, FileType};
use inkwell::OptimizationLevel;

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
}

pub struct CodeGen<'ctx> {
    pub context: &'ctx Context,
    pub module: Module<'ctx>,
    pub builder: Builder<'ctx>,
    pub shapes: std::cell::RefCell<HashMap<String, Vec<String>>>,
    pub warnings: std::cell::RefCell<Vec<String>>,
    pub templates: std::cell::RefCell<HashMap<String, (Vec<Param>, Expr)>>,
}

impl<'ctx> CodeGen<'ctx> {
    pub fn new(context: &'ctx Context, module_name: &str) -> Self {
        let module = context.create_module(module_name);
        let builder = context.create_builder();
        Self { 
            context, 
            module, 
            builder,
            shapes: std::cell::RefCell::new(HashMap::new()),
            warnings: std::cell::RefCell::new(Vec::new()),
            templates: std::cell::RefCell::new(HashMap::new()),
        }
    }

    pub fn gen_shape(&self, name: &str, fields: &[String]) {
        self.shapes.borrow_mut().insert(name.to_string(), fields.to_vec());
    }

    fn get_llvm_type(&self, name: &str) -> inkwell::types::BasicTypeEnum<'ctx> {
        match name {
            "i64" => self.context.i64_type().into(),
            "f64" => self.context.f64_type().into(),
            _ => self.context.i64_type().into(),
        }
    }

    pub fn gen_expr(&self, expr: &Expr, stack: &mut Vec<StackItem<'ctx>>, expand_map: &HashMap<String, usize>) -> BasicValueEnum<'ctx> {
        match expr {
            Expr::Integer(i) => self.context.i64_type().const_int(*i as u64, false).into(),
            Expr::Float(f) => self.context.f64_type().const_float(*f).into(),
            Expr::Identifier(name) => {
                if let Some(func) = self.module.get_function(name) {
                    func.as_global_value().as_pointer_value().into()
                } else {
                    panic!("E013");
                }
            }
            Expr::DeBruijn(index) => {
                if *index >= stack.len() {
                    panic!("E003");
                }
                let actual_index = stack.len() - 1 - index;
                let item = stack.get(actual_index).expect("E003");
                if item.state == VariableState::Moved {
                    panic!("E004");
                }
                item.value
            }
            Expr::BinaryOp(op, left, right) => {
                let lhs = self.gen_expr(left, stack, expand_map).into_int_value();
                let rhs = self.gen_expr(right, stack, expand_map).into_int_value();
                match op {
                    Token::Add => self.builder.build_int_add(lhs, rhs, "addtmp").unwrap().into(),
                    Token::Sub => self.builder.build_int_sub(lhs, rhs, "subtmp").unwrap().into(),
                    Token::Mul => self.builder.build_int_mul(lhs, rhs, "multmp").unwrap().into(),
                    _ => panic!("E008"),
                }
            }
            Expr::Move(inner) => {
                if let Expr::DeBruijn(index) = **inner {
                    let actual_index = stack.len() - 1 - index;
                    let val = {
                        let item = stack.get_mut(actual_index).expect("E003");
                        if item.state == VariableState::Moved {
                            panic!("E005");
                        }
                        item.state = VariableState::Moved;
                        item.value
                    };
                    val
                } else if let Expr::Expand(name) = &**inner {
                    let index = *expand_map.get(name).expect("E013");
                    let item = stack.get_mut(index).expect("E003");
                    if item.state == VariableState::Moved {
                        panic!("E005");
                    }
                    item.state = VariableState::Moved;
                    item.value
                } else {
                    self.gen_expr(inner, stack, expand_map)
                }
            }
            Expr::Borrow(inner) => {
                self.gen_expr(inner, stack, expand_map)
            }
            Expr::New(shape_name, count_expr) => {
                let count = self.gen_expr(count_expr, stack, expand_map).into_int_value();
                let shapes = self.shapes.borrow();
                let fields = shapes.get(shape_name).expect("E006");
                let mut field_ptrs = Vec::new();
                for field_type_name in fields {
                    let llvm_type = self.get_llvm_type(field_type_name);
                    let ptr = self.builder.build_array_alloca(llvm_type, count, "field_ptr").unwrap();
                    field_ptrs.push(ptr.into());
                }
                let struct_types: Vec<inkwell::types::BasicTypeEnum<'ctx>> = field_ptrs.iter().map(|v: &BasicValueEnum<'ctx>| v.get_type()).collect();
                let struct_type = self.context.struct_type(&struct_types, false);
                let mut struct_val = struct_type.get_undef();
                for (i, ptr) in field_ptrs.into_iter().enumerate() {
                    struct_val = self.builder.build_insert_value(struct_val, ptr, i as u32, "struct_insert").unwrap().into_struct_value();
                }
                struct_val.into()
            }
            Expr::Get(instance_expr, field_name, index_expr) => {
                let instance = self.gen_expr(instance_expr, stack, expand_map).into_struct_value();
                let index = self.gen_expr(index_expr, stack, expand_map).into_int_value();
                let shapes = self.shapes.borrow();
                let mut field_idx = 0;
                let mut found = false;
                let mut field_type_name = "i64";
                for (_, fields) in shapes.iter() {
                    if let Some(idx) = fields.iter().position(|f| f == field_name) {
                        field_idx = idx;
                        field_type_name = &fields[idx];
                        found = true;
                        break;
                    }
                }
                if !found { panic!("E007"); }
                let ptr_to_array = self.builder.build_extract_value(instance, field_idx as u32, "extract_ptr").unwrap().into_pointer_value();
                let llvm_field_type = self.get_llvm_type(field_type_name);
                let ptr_to_element = unsafe { self.builder.build_gep(llvm_field_type, ptr_to_array, &[index], "gep").unwrap() };
                self.builder.build_load(llvm_field_type, ptr_to_element, "load").unwrap()
            }
            Expr::Set(instance_expr, field_name, index_expr, value_expr) => {
                let instance = self.gen_expr(instance_expr, stack, expand_map).into_struct_value();
                let index = self.gen_expr(index_expr, stack, expand_map).into_int_value();
                let value = self.gen_expr(value_expr, stack, expand_map);
                let shapes = self.shapes.borrow();
                let mut field_idx = 0;
                let mut found = false;
                let mut field_type_name = "i64";
                for (_, fields) in shapes.iter() {
                    if let Some(idx) = fields.iter().position(|f| f == field_name) {
                        field_idx = idx;
                        field_type_name = &fields[idx];
                        found = true;
                        break;
                    }
                }
                if !found { panic!("E007"); }
                let llvm_field_type = self.get_llvm_type(field_type_name);
                let ptr_to_array = self.builder.build_extract_value(instance, field_idx as u32, "extract_ptr").unwrap().into_pointer_value();
                let ptr_to_element = unsafe { self.builder.build_gep(llvm_field_type, ptr_to_array, &[index], "gep").unwrap() };
                self.builder.build_store(ptr_to_element, value).unwrap();
                value
            }
            Expr::If(cond_expr, true_expr, false_expr) => {
                let cond = self.gen_expr(cond_expr, stack, expand_map).into_int_value();
                let parent_func = self.builder.get_insert_block().unwrap().get_parent().unwrap();
                let true_bb = self.context.append_basic_block(parent_func, "then");
                let false_bb = self.context.append_basic_block(parent_func, "else");
                let merge_bb = self.context.append_basic_block(parent_func, "ifcont");
                let zero = self.context.i64_type().const_int(0, false);
                let cond_bool = self.builder.build_int_compare(inkwell::IntPredicate::NE, cond, zero, "ifcond").unwrap();
                self.builder.build_conditional_branch(cond_bool, true_bb, false_bb).unwrap();
                let initial_stack_state: Vec<VariableState> = stack.iter().map(|item| item.state).collect();
                self.builder.position_at_end(true_bb);
                let true_val = self.gen_expr(true_expr, stack, expand_map);
                let true_stack_state: Vec<VariableState> = stack.iter().map(|item| item.state).collect();
                self.builder.build_unconditional_branch(merge_bb).unwrap();
                let true_bb_final = self.builder.get_insert_block().unwrap();
                for (i, state) in initial_stack_state.iter().enumerate() {
                    stack[i].state = *state;
                }
                self.builder.position_at_end(false_bb);
                let false_val = self.gen_expr(false_expr, stack, expand_map);
                let false_stack_state: Vec<VariableState> = stack.iter().map(|item| item.state).collect();
                self.builder.build_unconditional_branch(merge_bb).unwrap();
                let false_bb_final = self.builder.get_insert_block().unwrap();
                if true_stack_state != false_stack_state {
                    panic!("E009");
                }
                self.builder.position_at_end(merge_bb);
                let phi = self.builder.build_phi(true_val.get_type(), "iftmp").unwrap();
                phi.add_incoming(&[(&true_val, true_bb_final), (&false_val, false_bb_final)]);
                phi.as_basic_value()
            }
            Expr::Apply(func_expr, args) => {
                if let Expr::Identifier(ref name) = **func_expr {
                    let template_opt = self.templates.borrow().get(name).cloned();
                    if let Some((params, body)) = template_opt {
                        let mut args_vals = Vec::new();
                        for arg in args {
                            args_vals.push(self.gen_expr(arg, stack, expand_map));
                        }
                        
                        let initial_stack_len = stack.len();
                        let mut new_expand_map = HashMap::new();
                        for (i, (param, val)) in params.iter().zip(args_vals.into_iter()).enumerate() {
                            stack.push(StackItem { value: val, state: VariableState::Available });
                            if param.expand {
                                new_expand_map.insert(param.name.clone(), stack.len() - 1);
                            }
                        }
                        
                        let res = self.gen_expr(&body, stack, &new_expand_map);
                        stack.truncate(initial_stack_len);
                        res
                    } else {
                        let function = self.module.get_function(name).expect("E010");
                        let mut args_vals = Vec::new();
                        for arg in args {
                            args_vals.push(self.gen_expr(arg, stack, expand_map).into());
                        }
                        let call = self.builder.build_call(function, &args_vals, "calltmp").unwrap();
                        call.try_as_basic_value().basic().expect("E011")
                    }
                } else {
                    panic!("E012");
                }
            }
            Expr::Expand(name) => {
                let stack_index = *expand_map.get(name).expect("E013");
                let item = stack.get(stack_index).expect("E003");
                if item.state == VariableState::Moved {
                    panic!("E004");
                }
                item.value
            }
            Expr::Let(_name, val_expr, body_expr) => {
                let val = self.gen_expr(val_expr, stack, expand_map);
                stack.push(StackItem { value: val, state: VariableState::Available });
                let res = self.gen_expr(body_expr, stack, expand_map);
                let item = stack.pop().unwrap();
                if item.state == VariableState::Available {
                    self.warnings.borrow_mut().push("W001".to_string());
                }
                res
            }
            _ => panic!("E001"),
        }
    }

    pub fn gen_import(&self, _module_alias: &str, symbol_name: &str) {
        // Register an external function signature
        let i64_type = self.context.i64_type();
        let fn_type = i64_type.fn_type(&[i64_type.into()], false);
        self.module.add_function(symbol_name, fn_type, None);
    }

    pub fn gen_function(&self, name: &str, params: Vec<Param>, body: &Expr) -> FunctionValue<'ctx> {
        if params.iter().any(|p| p.expand) {
            self.templates.borrow_mut().insert(name.to_string(), (params, body.clone()));
            let fn_type = self.context.i64_type().fn_type(&[], false);
            return self.module.add_function(name, fn_type, None);
        }

        let arg_count = params.len();
        let i64_type = self.context.i64_type();
        let args_types = vec![i64_type.into(); arg_count];
        let fn_type = i64_type.fn_type(&args_types, false);
        let function = self.module.add_function(name, fn_type, None);
        let basic_block = self.context.append_basic_block(function, "entry");
        self.builder.position_at_end(basic_block);
        let mut stack = Vec::new();
        for i in 0..arg_count {
            stack.push(StackItem {
                value: function.get_nth_param(i as u32).unwrap(),
                state: VariableState::Available,
            });
        }
        let ret_val = self.gen_expr(body, &mut stack, &HashMap::new());
        for item in stack.iter() {
            if item.state == VariableState::Available {
                self.warnings.borrow_mut().push("W001".to_string());
            }
        }
        self.builder.build_return(Some(&ret_val)).unwrap();
        function
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
        let shapes = self.shapes.borrow();
        for (name, fields) in shapes.iter() {
            sig.push_str(&format!("# {} {}\n", name, fields.join(" ")));
        }
        for func in self.module.get_functions() {
            let name = func.get_name().to_str().unwrap();
            if name == "main" { continue; }
            sig.push_str(&format!(": {} ...\n", name));
        }
        sig
    }
}
