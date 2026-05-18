use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::builder::Builder;
use inkwell::values::{FunctionValue, BasicValueEnum};
use crate::parser::Expr;
use crate::lexer::Token;
use std::collections::HashMap;

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

    pub fn gen_expr(&self, expr: &Expr, stack: &mut Vec<StackItem<'ctx>>) -> BasicValueEnum<'ctx> {
        match expr {
            Expr::Integer(i) => self.context.i64_type().const_int(*i as u64, false).into(),
            Expr::Float(f) => self.context.f64_type().const_float(*f).into(),
            Expr::Identifier(name) => {
                // If it's just an identifier, check if it's a function name in the module
                if let Some(func) = self.module.get_function(name) {
                    func.as_global_value().as_pointer_value().into()
                } else {
                    panic!("E013"); // Unknown identifier
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
                let lhs = self.gen_expr(left, stack).into_int_value();
                let rhs = self.gen_expr(right, stack).into_int_value();
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
                } else {
                    self.gen_expr(inner, stack)
                }
            }
            Expr::Borrow(inner) => {
                self.gen_expr(inner, stack)
            }
            Expr::New(shape_name, count_expr) => {
                let count = self.gen_expr(count_expr, stack).into_int_value();
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
                let instance = self.gen_expr(instance_expr, stack).into_struct_value();
                let index = self.gen_expr(index_expr, stack).into_int_value();
                
                let shapes = self.shapes.borrow();
                let mut field_idx = 0;
                let mut found = false;
                for (_, fields) in shapes.iter() {
                    if let Some(idx) = fields.iter().position(|f| f == field_name) {
                        field_idx = idx;
                        found = true;
                        break;
                    }
                }
                if !found { panic!("E007"); }

                let ptr_to_array = self.builder.build_extract_value(instance, field_idx as u32, "extract_ptr").unwrap().into_pointer_value();
                
                let mut field_type_name = "i64";
                for (_, fields) in shapes.iter() {
                    if let Some(idx) = fields.iter().position(|f| f == field_name) {
                        field_type_name = &fields[idx];
                        break;
                    }
                }
                let llvm_field_type = self.get_llvm_type(field_type_name);

                let ptr_to_element = unsafe { self.builder.build_gep(llvm_field_type, ptr_to_array, &[index], "gep").unwrap() };
                self.builder.build_load(llvm_field_type, ptr_to_element, "load").unwrap()
            }
            Expr::Set(instance_expr, field_name, index_expr, value_expr) => {
                let instance = self.gen_expr(instance_expr, stack).into_struct_value();
                let index = self.gen_expr(index_expr, stack).into_int_value();
                let value = self.gen_expr(value_expr, stack);

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
                let cond = self.gen_expr(cond_expr, stack).into_int_value();
                let parent_func = self.builder.get_insert_block().unwrap().get_parent().unwrap();

                let true_bb = self.context.append_basic_block(parent_func, "then");
                let false_bb = self.context.append_basic_block(parent_func, "else");
                let merge_bb = self.context.append_basic_block(parent_func, "ifcont");

                let zero = self.context.i64_type().const_int(0, false);
                let cond_bool = self.builder.build_int_compare(inkwell::IntPredicate::NE, cond, zero, "ifcond").unwrap();
                self.builder.build_conditional_branch(cond_bool, true_bb, false_bb).unwrap();

                let initial_stack_state: Vec<VariableState> = stack.iter().map(|item| item.state).collect();
                
                self.builder.position_at_end(true_bb);
                let true_val = self.gen_expr(true_expr, stack);
                let true_stack_state: Vec<VariableState> = stack.iter().map(|item| item.state).collect();
                self.builder.build_unconditional_branch(merge_bb).unwrap();
                let true_bb_final = self.builder.get_insert_block().unwrap();

                for (i, state) in initial_stack_state.iter().enumerate() {
                    stack[i].state = *state;
                }

                self.builder.position_at_end(false_bb);
                let false_val = self.gen_expr(false_expr, stack);
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
                    let function = self.module.get_function(name).expect("E010");
                    let mut args_vals = Vec::new();
                    for arg in args {
                        args_vals.push(self.gen_expr(arg, stack).into());
                    }
                    let call = self.builder.build_call(function, &args_vals, "calltmp").unwrap();
                    call.try_as_basic_value().basic().expect("E011")
                } else {
                    panic!("E012");
                }
            }
            _ => panic!("E001"),
        }
    }

    pub fn gen_function(&self, name: &str, arg_count: usize, body: &Expr) -> FunctionValue<'ctx> {
        let i64_type = self.context.i64_type();
        let args_types = vec![i64_type.into(); arg_count];
        let fn_type = i64_type.fn_type(&args_types, false);
        
        // Register function signature BEFORE generating body to allow recursion
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
        
        let ret_val = self.gen_expr(body, &mut stack);
        
        for item in stack.iter() {
            if item.state == VariableState::Available {
                self.warnings.borrow_mut().push("W001".to_string());
            }
        }

        self.builder.build_return(Some(&ret_val)).unwrap();
        
        function
    }
}
