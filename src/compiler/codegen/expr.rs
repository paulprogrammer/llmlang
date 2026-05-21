use crate::compiler::ast::Expr;
use crate::compiler::lexer::Token;
use crate::compiler::codegen::{CodeGen, StackItem, VariableState};
use inkwell::values::BasicValueEnum;
use std::collections::HashMap;

impl<'ctx> CodeGen<'ctx> {
    pub fn gen_expr(&self, expr: &Expr, stack: &mut Vec<StackItem<'ctx>>, expand_map: &HashMap<String, usize>) -> BasicValueEnum<'ctx> {
        match expr {
            Expr::Integer(i) => self.context.i64_type().const_int(*i as u64, false).into(),
            Expr::Float(f) => self.context.f64_type().const_float(*f).into(),
            Expr::String(s) => self.gen_string_constant(s),
            Expr::Identifier(name) => {
                let resolved = self.resolve_func_name(name);
                if let Some(func) = self.module.get_function(&resolved) {
                    func.as_global_value().as_pointer_value().into()
                } else if let Some(func) = self.module.get_function(name) {
                    func.as_global_value().as_pointer_value().into()
                } else {
                    panic!("E013: Unknown identifier {}", name);
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
                let parallel_threshold = self.config.parallel_threshold;
                let mut left_handle = None;
                if left.is_pure() && left.complexity() > parallel_threshold {
                    left_handle = Some(self.gen_parallel_expr(left, stack, expand_map));
                }
                let lhs_val = if let Some(handle) = left_handle {
                    let join_fn_type = self.context.i64_type().fn_type(&[self.context.i64_type().into()], false);
                    let join_fn = self.get_or_add_external_fn("llm_join", join_fn_type);
                    let call = self.builder.build_call(join_fn, &[handle.into()], "join_res").unwrap();
                    self.get_call_res(call)
                } else {
                    self.gen_expr(left, stack, expand_map)
                };
                let rhs_val = self.gen_expr(right, stack, expand_map);

                if lhs_val.is_float_value() || rhs_val.is_float_value() {
                    let lhs = if lhs_val.is_int_value() {
                        self.builder.build_signed_int_to_float(lhs_val.into_int_value(), self.context.f64_type(), "fpromote").unwrap()
                    } else {
                        lhs_val.into_float_value()
                    };
                    let rhs = if rhs_val.is_int_value() {
                        self.builder.build_signed_int_to_float(rhs_val.into_int_value(), self.context.f64_type(), "fpromote").unwrap()
                    } else {
                        rhs_val.into_float_value()
                    };

                    match op {
                        Token::Add => self.builder.build_float_add(lhs, rhs, "fadd").unwrap().into(),
                        Token::Sub => self.builder.build_float_sub(lhs, rhs, "fsub").unwrap().into(),
                        Token::Mul => self.builder.build_float_mul(lhs, rhs, "fmul").unwrap().into(),
                        Token::Div => self.builder.build_float_div(lhs, rhs, "fdiv").unwrap().into(),
                        Token::Eq => {
                            let cmp = self.builder.build_float_compare(inkwell::FloatPredicate::OEQ, lhs, rhs, "feq").unwrap();
                            self.builder.build_int_z_extend(cmp, self.context.i64_type(), "zext").unwrap().into()
                        }
                        Token::Lt => {
                            let cmp = self.builder.build_float_compare(inkwell::FloatPredicate::OLT, lhs, rhs, "flt").unwrap();
                            self.builder.build_int_z_extend(cmp, self.context.i64_type(), "zext").unwrap().into()
                        }
                        Token::Gt => {
                            let cmp = self.builder.build_float_compare(inkwell::FloatPredicate::OGT, lhs, rhs, "fgt").unwrap();
                            self.builder.build_int_z_extend(cmp, self.context.i64_type(), "zext").unwrap().into()
                        }
                        _ => panic!("E008"),
                    }
                } else {
                    let lhs = self.as_int(lhs_val);
                    let rhs = self.as_int(rhs_val);
                    match op {
                        Token::Add => self.builder.build_int_add(lhs, rhs, "add").unwrap().into(),
                        Token::Sub => self.builder.build_int_sub(lhs, rhs, "sub").unwrap().into(),
                        Token::Mul => self.builder.build_int_mul(lhs, rhs, "mul").unwrap().into(),
                        Token::Div => self.builder.build_int_signed_div(lhs, rhs, "div").unwrap().into(),
                        Token::Eq => {
                            let cmp = self.builder.build_int_compare(inkwell::IntPredicate::EQ, lhs, rhs, "eq").unwrap();
                            self.builder.build_int_z_extend(cmp, self.context.i64_type(), "zext").unwrap().into()
                        }
                        Token::Lt => {
                            let cmp = self.builder.build_int_compare(inkwell::IntPredicate::SLT, lhs, rhs, "lt").unwrap();
                            self.builder.build_int_z_extend(cmp, self.context.i64_type(), "zext").unwrap().into()
                        }
                        Token::Gt => {
                            let cmp = self.builder.build_int_compare(inkwell::IntPredicate::SGT, lhs, rhs, "gt").unwrap();
                            self.builder.build_int_z_extend(cmp, self.context.i64_type(), "zext").unwrap().into()
                        }
                        Token::BitAnd => self.builder.build_and(lhs, rhs, "and").unwrap().into(),
                        Token::BitOr => self.builder.build_or(lhs, rhs, "or").unwrap().into(),
                        Token::BitXor => self.builder.build_xor(lhs, rhs, "xor").unwrap().into(),
                        _ => panic!("E008"),
                    }
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
                        if item.state == VariableState::Borrowed {
                            panic!("E016");
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
                    if item.state == VariableState::Borrowed {
                        panic!("E016");
                    }
                    item.state = VariableState::Moved;
                    item.value
                } else {
                    self.gen_expr(inner, stack, expand_map)
                }
            }
            Expr::Borrow(inner) | Expr::MutBorrow(inner) => self.gen_expr(inner, stack, expand_map),
            Expr::New(shape_name, count_expr) => {
                let count_val = self.gen_expr(count_expr, stack, expand_map);
                let count = self.as_int(count_val);
                let shapes = self.shapes.borrow();
                let fields = shapes.get(shape_name).expect("E006");
                let mut members: Vec<BasicValueEnum<'ctx>> = Vec::new();
                members.push(count.into());
                for _field_type_name in fields {
                    let size_bytes = self.builder.build_int_mul(count, self.context.i64_type().const_int(8, false), "size").unwrap();
                    let fn_type = self.context.ptr_type(inkwell::AddressSpace::default()).fn_type(&[self.context.i64_type().into()], false);
                    let func = self.get_or_add_external_fn("llm_alloc", fn_type);
                    let call = self.builder.build_call(func, &[size_bytes.into()], "col_ptr_raw").unwrap();
                    let ptr_val = self.get_call_res(call);
                    let ptr = self.builder.build_ptr_to_int(ptr_val.into_pointer_value(), self.context.i64_type(), "col_ptr").unwrap();
                    members.push(ptr.into());
                }
                let struct_size = (members.len() as u64) * 8;
                let fn_type = self.context.ptr_type(inkwell::AddressSpace::default()).fn_type(&[self.context.i64_type().into()], false);
                let func = self.get_or_add_external_fn("llm_alloc", fn_type);
                let call = self.builder.build_call(func, &[self.context.i64_type().const_int(struct_size, false).into()], "struct_ptr_raw").unwrap();
                let struct_ptr_raw = self.get_call_res(call).into_pointer_value();
                let struct_ptr_int = self.builder.build_ptr_to_int(struct_ptr_raw, self.context.i64_type(), "struct_ptr").unwrap();
                for (i, val) in members.into_iter().enumerate() {
                    let member_ptr = unsafe { self.builder.build_gep(self.context.i64_type(), struct_ptr_raw, &[self.context.i64_type().const_int(i as u64, false)], "member_ptr").unwrap() };
                    self.builder.build_store(member_ptr, val).unwrap();
                }
                struct_ptr_int.into()
            }
            Expr::Get(instance_expr, field_name, index_expr) => {
                let struct_ptr_val = self.gen_expr(instance_expr, stack, expand_map);
                let struct_ptr_int = self.as_int(struct_ptr_val);
                let struct_ptr = self.builder.build_int_to_ptr(struct_ptr_int, self.context.ptr_type(inkwell::AddressSpace::default()), "struct_ptr").unwrap();
                let index_val = self.gen_expr(index_expr, stack, expand_map);
                let index = self.as_int(index_val);
                let shapes = self.shapes.borrow();
                let mut field_idx = 0;
                let mut found = false;
                let mut field_type_name = "i64";

                let inferred = self.infer_shape(instance_expr, stack);
                if let Some(ref shape_name) = inferred {
                    if let Some(fields) = shapes.get(shape_name) {
                        if let Some(idx) = fields.iter().position(|f| f == field_name) {
                            field_idx = idx + 1;
                            field_type_name = &fields[idx];
                            found = true;
                        }
                    }
                } else {
                    for (_, fields) in shapes.iter() {
                        if let Some(idx) = fields.iter().position(|f| f == field_name) {
                            field_idx = idx + 1;
                            field_type_name = &fields[idx];
                            found = true;
                            break;
                        }
                    }
                }

                if !found { panic!("E007"); }
                let llvm_field_type = self.get_llvm_type(field_type_name);
                let col_ptr_ptr = unsafe { self.builder.build_gep(self.context.i64_type(), struct_ptr, &[self.context.i64_type().const_int(field_idx as u64, false)], "col_ptr_ptr").unwrap() };
                let col_ptr_int_val = self.builder.build_load(self.context.i64_type(), col_ptr_ptr, "col_ptr_int").unwrap();
                let col_ptr_int = self.as_int(col_ptr_int_val);
                let col_ptr = self.builder.build_int_to_ptr(col_ptr_int, self.context.ptr_type(inkwell::AddressSpace::default()), "col_ptr").unwrap();
                let ptr_to_element = unsafe { self.builder.build_gep(llvm_field_type, col_ptr, &[index], "gep").unwrap() };
                let loaded = self.builder.build_load(llvm_field_type, ptr_to_element, "load").unwrap();
                self.maybe_drop_val(instance_expr, struct_ptr_val, stack);
                loaded
            }
            Expr::Set(instance_expr, field_name, index_expr, value_expr) => {
                let struct_ptr_val = self.gen_expr(instance_expr, stack, expand_map);
                let struct_ptr_int = self.as_int(struct_ptr_val);
                let struct_ptr = self.builder.build_int_to_ptr(struct_ptr_int, self.context.ptr_type(inkwell::AddressSpace::default()), "struct_ptr").unwrap();
                let index_val = self.gen_expr(index_expr, stack, expand_map);
                let index = self.as_int(index_val);
                let value = self.gen_expr(value_expr, stack, expand_map);
                let shapes = self.shapes.borrow();
                let mut field_idx = 0;
                let mut found = false;
                let mut field_type_name = "i64";

                let inferred = self.infer_shape(instance_expr, stack);
                if let Some(ref shape_name) = inferred {
                    if let Some(fields) = shapes.get(shape_name) {
                        if let Some(idx) = fields.iter().position(|f| f == field_name) {
                            field_idx = idx + 1;
                            field_type_name = &fields[idx];
                            found = true;
                        }
                    }
                } else {
                    for (_, fields) in shapes.iter() {
                        if let Some(idx) = fields.iter().position(|f| f == field_name) {
                            field_idx = idx + 1;
                            field_type_name = &fields[idx];
                            found = true;
                            break;
                        }
                    }
                }

                if !found { panic!("E007"); }
                let llvm_field_type = self.get_llvm_type(field_type_name);
                let col_ptr_ptr = unsafe { self.builder.build_gep(self.context.i64_type(), struct_ptr, &[self.context.i64_type().const_int(field_idx as u64, false)], "col_ptr_ptr").unwrap() };
                let col_ptr_int_val = self.builder.build_load(self.context.i64_type(), col_ptr_ptr, "col_ptr_int").unwrap();
                let col_ptr_int = self.as_int(col_ptr_int_val);
                let col_ptr = self.builder.build_int_to_ptr(col_ptr_int, self.context.ptr_type(inkwell::AddressSpace::default()), "col_ptr").unwrap();
                let ptr_to_element = unsafe { self.builder.build_gep(llvm_field_type, col_ptr, &[index], "gep").unwrap() };
                self.builder.build_store(ptr_to_element, value).unwrap();
                self.maybe_drop_val(instance_expr, struct_ptr_val, stack);
                value
            }
            Expr::If(cond_expr, true_expr, false_expr) => {
                let cond_val = self.gen_expr(cond_expr, stack, expand_map);
                let cond = self.as_int(cond_val);
                let parent_func = self.builder.get_insert_block().unwrap().get_parent().unwrap();
                let true_bb = self.context.append_basic_block(parent_func, "then");
                let false_bb = self.context.append_basic_block(parent_func, "else");
                let merge_bb = self.context.append_basic_block(parent_func, "ifcont");
                let zero = self.context.i64_type().const_int(0, false);
                let cond_bool = self.builder.build_int_compare(inkwell::IntPredicate::NE, cond, zero, "ifcond").unwrap();
                self.builder.build_conditional_branch(cond_bool, true_bb, false_bb).unwrap();
                let initial_stack_state: Vec<VariableState> = stack.iter().map(|item| item.state).collect();
                let initial_shapes: Vec<Option<String>> = stack.iter().map(|item| item.shape.clone()).collect();
                let initial_is_ptr: Vec<bool> = stack.iter().map(|item| item.is_ptr).collect();
                self.builder.position_at_end(true_bb);
                let true_val = self.gen_expr(true_expr, stack, expand_map);
                let true_stack_state: Vec<VariableState> = stack.iter().map(|item| item.state).collect();
                self.builder.build_unconditional_branch(merge_bb).unwrap();
                let true_bb_final = self.builder.get_insert_block().unwrap();
                for (i, state) in initial_stack_state.iter().enumerate() {
                    stack[i].state = *state;
                    stack[i].shape = initial_shapes[i].clone();
                    stack[i].is_ptr = initial_is_ptr[i];
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
                    let final_func_name = self.resolve_func_name(name);

                    let template_opt = self.templates.borrow().get(&final_func_name).cloned();
                    let mut args_vals = Vec::new();
                    let mut handles = Vec::new();
                    let parallel_threshold = self.config.parallel_threshold;
                    for (i, arg) in args.iter().enumerate() {
                        if i < args.len() - 1 && arg.is_pure() && arg.complexity() > parallel_threshold {
                            handles.push((i, self.gen_parallel_expr(arg, stack, expand_map)));
                            args_vals.push(self.context.i64_type().const_int(0, false).into());
                        } else {
                            args_vals.push(self.gen_expr(arg, stack, expand_map));
                        }
                    }
                    if !handles.is_empty() {
                        let join_fn_type = self.context.i64_type().fn_type(&[self.context.i64_type().into()], false);
                        let join_fn = self.get_or_add_external_fn("llm_join", join_fn_type);
                        for (idx, handle) in handles {
                            let call = self.builder.build_call(join_fn, &[handle.into()], "arg_join").unwrap();
                            let res = self.get_call_res(call);
                            args_vals[idx] = res;
                        }
                    }
                    if let Some((params, body)) = template_opt {
                        let initial_stack_len = stack.len();
                        let mut new_expand_map = HashMap::new();
                        let mut is_owned = Vec::new();
                        for (param, (val, arg_expr)) in params.iter().zip(args_vals.into_iter().zip(args.iter())) {
                            let stack_ptrs: Vec<bool> = stack.iter().map(|item| item.is_ptr).collect();
                            let is_ptr = arg_expr.returns_ptr_with_stack(&stack_ptrs, &self.fn_returns_ptr.borrow());
                            stack.push(StackItem { value: val, state: VariableState::Available, shape: self.infer_shape(arg_expr, stack), is_ptr });
                            is_owned.push(self.is_owned_ptr(arg_expr, stack));
                            if param.expand {
                                new_expand_map.insert(param.name.clone(), stack.len() - 1);
                            }
                        }
                        let res = self.gen_expr(&body, stack, &new_expand_map);
                        for (param_item, owned) in stack.drain(initial_stack_len..).zip(is_owned.into_iter()) {
                            if owned && param_item.state == VariableState::Available {
                                self.emit_auto_drop(param_item.value, param_item.shape.as_deref(), param_item.is_ptr);
                            }
                        }
                        res
                    } else {
                        let function = self.module.get_function(&final_func_name).expect("E010");
                        let mut call_args = Vec::new();
                        
                        let is_ffi = if let Some(module) = self.imports.borrow().get(name) {
                            module == "crypto" || module == "cms" || module == "file" || module == "http" || module == "json"
                        } else {
                            false
                        };
                        let is_user_defined = !is_ffi;

                        for (arg, val) in args.iter().zip(args_vals.clone().into_iter()) {
                            let mut final_val = val;
                            if is_user_defined && matches!(arg, Expr::Borrow(_) | Expr::MutBorrow(_)) {
                                let stack_ptrs: Vec<bool> = stack.iter().map(|item| item.is_ptr).collect();
                                if arg.returns_ptr_with_stack(&stack_ptrs, &self.fn_returns_ptr.borrow()) {
                                    let dup_fn_type = self.context.i64_type().fn_type(&[self.context.i64_type().into()], false);
                                    let dup_fn = self.get_or_add_external_fn("llm_dup", dup_fn_type);
                                    let call = self.builder.build_call(dup_fn, &[val.into()], "dup_val").unwrap();
                                    final_val = self.get_call_res(call);
                                }
                            }
                            call_args.push(final_val.into());
                        }
                        let call = self.builder.build_call(function, &call_args, "calltmp").unwrap();
                        call.set_tail_call(true);
                        let res = self.get_call_res(call);
                        if is_ffi {
                            for (arg, val) in args.iter().zip(args_vals.into_iter()) {
                                self.maybe_drop_val(arg, val, stack);
                            }
                        }
                        res
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
                let shape = self.infer_shape(val_expr, stack);
                let stack_ptrs: Vec<bool> = stack.iter().map(|item| item.is_ptr).collect();
                let is_ptr = val_expr.returns_ptr_with_stack(&stack_ptrs, &self.fn_returns_ptr.borrow());
                stack.push(StackItem { value: val, state: VariableState::Available, shape, is_ptr });
                let res = self.gen_expr(body_expr, stack, expand_map);
                let item = stack.pop().unwrap();
                if item.state == VariableState::Available {
                    self.emit_auto_drop(item.value, item.shape.as_deref(), item.is_ptr);
                }
                res
            }
            Expr::Len(e) => {
                let s_val = self.gen_expr(e, stack, expand_map);
                if let Some(_shape) = self.infer_shape(e, stack) {
                    let s_int = self.as_int(s_val);
                    let ptr = self.builder.build_int_to_ptr(s_int, self.context.ptr_type(inkwell::AddressSpace::default()), "soa_ptr").unwrap();
                    self.builder.build_load(self.context.i64_type(), ptr, "count").unwrap()
                } else {
                    let fn_type = self.context.i64_type().fn_type(&[self.context.i64_type().into()], false);
                    let func = self.get_or_add_external_fn("llm_len", fn_type);
                    let call = self.builder.build_call(func, &[s_val.into()], "len").unwrap();
                    self.get_call_res(call)
                }
            }
            Expr::Cat(l, r) => {
                let l_val = self.gen_expr(l, stack, expand_map);
                let r_val = self.gen_expr(r, stack, expand_map);
                let fn_type = self.context.i64_type().fn_type(&[self.context.i64_type().into(), self.context.i64_type().into()], false);
                let func = self.get_or_add_external_fn("llm_cat", fn_type);
                let call = self.builder.build_call(func, &[l_val.into(), r_val.into()], "cat").unwrap();
                let res = self.get_call_res(call);
                self.maybe_drop_val(l, l_val, stack);
                self.maybe_drop_val(r, r_val, stack);
                res
            }
            Expr::Sub(s, b, l) => {
                let s_val = self.gen_expr(s, stack, expand_map);
                let b_val = self.gen_expr(b, stack, expand_map);
                let l_val = self.gen_expr(l, stack, expand_map);
                let fn_type = self.context.i64_type().fn_type(&[
                    self.context.i64_type().into(),
                    self.context.i64_type().into(),
                    self.context.i64_type().into(),
                ], false);
                let func = self.get_or_add_external_fn("llm_sub", fn_type);
                let call = self.builder.build_call(func, &[s_val.into(), b_val.into(), l_val.into()], "sub").unwrap();
                let res = self.get_call_res(call);
                self.maybe_drop_val(s, s_val, stack);
                res
            }
            Expr::Loc(s, p) => {
                let s_val = self.gen_expr(s, stack, expand_map);
                let p_val = self.gen_expr(p, stack, expand_map);
                let fn_type = self.context.i64_type().fn_type(&[self.context.i64_type().into(), self.context.i64_type().into()], false);
                let func = self.get_or_add_external_fn("llm_loc", fn_type);
                let call = self.builder.build_call(func, &[s_val.into(), p_val.into()], "loc").unwrap();
                let res = self.get_call_res(call);
                self.maybe_drop_val(s, s_val, stack);
                self.maybe_drop_val(p, p_val, stack);
                res
            }
            Expr::Reg(s, r) => {
                let s_val = self.gen_expr(s, stack, expand_map);
                let r_val = self.gen_expr(r, stack, expand_map);
                let fn_type = self.context.i64_type().fn_type(&[self.context.i64_type().into(), self.context.i64_type().into()], false);
                let func = self.get_or_add_external_fn("llm_reg", fn_type);
                let call = self.builder.build_call(func, &[s_val.into(), r_val.into()], "reg").unwrap();
                let res = self.get_call_res(call);
                self.maybe_drop_val(s, s_val, stack);
                self.maybe_drop_val(r, r_val, stack);
                res
            }
            Expr::Read(h) => {
                let h_val = self.gen_expr(h, stack, expand_map);
                let fn_type = self.context.i64_type().fn_type(&[self.context.i64_type().into()], false);
                let func = self.get_or_add_external_fn("llm_read", fn_type);
                let call = self.builder.build_call(func, &[h_val.into()], "read").unwrap();
                let res = self.get_call_res(call);
                self.maybe_drop_val(h, h_val, stack);
                res
            }
            Expr::Write(h, s) => {
                let h_val = self.gen_expr(h, stack, expand_map);
                let s_val = self.gen_expr(s, stack, expand_map);
                let fn_type = self.context.i64_type().fn_type(&[self.context.i64_type().into(), self.context.i64_type().into()], false);
                let func = self.get_or_add_external_fn("llm_write", fn_type);
                let call = self.builder.build_call(func, &[h_val.into(), s_val.into()], "write").unwrap();
                let res = self.get_call_res(call);
                self.maybe_drop_val(h, h_val, stack);
                self.maybe_drop_val(s, s_val, stack);
                res
            }
            Expr::Str(e) => {
                let val = self.gen_expr(e, stack, expand_map);
                let fn_type = self.context.i64_type().fn_type(&[self.context.i64_type().into()], false);
                let func = self.get_or_add_external_fn("llm_itoa", fn_type);
                let call = self.builder.build_call(func, &[val.into()], "itoa").unwrap();
                let res = self.get_call_res(call);
                self.maybe_drop_val(e, val, stack);
                res
            }
            Expr::Split(s, d, i) => {
                let s_val = self.gen_expr(s, stack, expand_map);
                let d_val = self.gen_expr(d, stack, expand_map);
                let i_val = self.gen_expr(i, stack, expand_map);
                let fn_type = self.context.i64_type().fn_type(&[self.context.i64_type().into(), self.context.i64_type().into(), self.context.i64_type().into()], false);
                let func = self.get_or_add_external_fn("llm_split", fn_type);
                let call = self.builder.build_call(func, &[s_val.into(), d_val.into(), i_val.into()], "split").unwrap();
                let res = self.get_call_res(call);
                self.maybe_drop_val(s, s_val, stack);
                self.maybe_drop_val(d, d_val, stack);
                res
            }
            Expr::TimeNow => {
                let fn_type = self.context.i64_type().fn_type(&[], false);
                let func = self.get_or_add_external_fn("llm_tai_now", fn_type);
                let call = self.builder.build_call(func, &[], "now").unwrap();
                self.get_call_res(call)
            }
            Expr::TimeNano => {
                let fn_type = self.context.i64_type().fn_type(&[], false);
                let func = self.get_or_add_external_fn("llm_tai_nano", fn_type);
                let call = self.builder.build_call(func, &[], "nano").unwrap();
                self.get_call_res(call)
            }
            Expr::TimeGet(t, i) => {
                let t_val = self.gen_expr(t, stack, expand_map);
                let i_val = self.gen_expr(i, stack, expand_map);
                let fn_type = self.context.i64_type().fn_type(&[self.context.i64_type().into(), self.context.i64_type().into()], false);
                let func = self.get_or_add_external_fn("llm_tai_get", fn_type);
                let call = self.builder.build_call(func, &[t_val.into(), i_val.into()], "get").unwrap();
                let res = self.get_call_res(call);
                self.maybe_drop_val(t, t_val, stack);
                res
            }
            Expr::TimeSet(y, m, d, h, mn, s) => {
                let y_val = self.gen_expr(y, stack, expand_map);
                let m_val = self.gen_expr(m, stack, expand_map);
                let d_val = self.gen_expr(d, stack, expand_map);
                let h_val = self.gen_expr(h, stack, expand_map);
                let mn_val = self.gen_expr(mn, stack, expand_map);
                let s_val = self.gen_expr(s, stack, expand_map);
                let i64_type = self.context.i64_type();
                let fn_type = i64_type.fn_type(&[i64_type.into(); 6], false);
                let func = self.get_or_add_external_fn("llm_tai_set", fn_type);
                let call = self.builder.build_call(func, &[y_val.into(), m_val.into(), d_val.into(), h_val.into(), mn_val.into(), s_val.into()], "set").unwrap();
                let res = self.get_call_res(call);
                self.maybe_drop_val(y, y_val, stack);
                self.maybe_drop_val(m, m_val, stack);
                self.maybe_drop_val(d, d_val, stack);
                self.maybe_drop_val(h, h_val, stack);
                self.maybe_drop_val(mn, mn_val, stack);
                self.maybe_drop_val(s, s_val, stack);
                res
            }
            Expr::Env(k) => {
                let k_val = self.gen_expr(k, stack, expand_map);
                let fn_type = self.context.i64_type().fn_type(&[self.context.i64_type().into()], false);
                let func = self.get_or_add_external_fn("llm_getenv", fn_type);
                let call = self.builder.build_call(func, &[k_val.into()], "getenv").unwrap();
                let res = self.get_call_res(call);
                self.maybe_drop_val(k, k_val, stack);
                res
            }
            Expr::HttpClient(method, url, body) => {
                let method_val = self.gen_expr(method, stack, expand_map);
                let url_val = self.gen_expr(url, stack, expand_map);
                let body_val = self.gen_expr(body, stack, expand_map);
                let fn_type = self.context.i64_type().fn_type(&[
                    self.context.i64_type().into(),
                    self.context.i64_type().into(),
                    self.context.i64_type().into(),
                ], false);
                let func = self.get_or_add_external_fn("llm_http_client", fn_type);
                let call = self.builder.build_call(func, &[method_val.into(), url_val.into(), body_val.into()], "http_client").unwrap();
                let res = self.get_call_res(call);
                self.maybe_drop_val(method, method_val, stack);
                self.maybe_drop_val(url, url_val, stack);
                self.maybe_drop_val(body, body_val, stack);
                res
            }
            Expr::HttpServer(op, arg) => {
                let op_val = self.gen_expr(op, stack, expand_map);
                let arg_val = self.gen_expr(arg, stack, expand_map);
                let fn_type = self.context.i64_type().fn_type(&[
                    self.context.i64_type().into(),
                    self.context.i64_type().into(),
                ], false);
                let func = self.get_or_add_external_fn("llm_http_server", fn_type);
                let call = self.builder.build_call(func, &[op_val.into(), arg_val.into()], "http_server").unwrap();
                let res = self.get_call_res(call);
                self.maybe_drop_val(op, op_val, stack);
                self.maybe_drop_val(arg, arg_val, stack);
                res
            }
            Expr::FileOpen(path, mode) => {
                let path_val = self.gen_expr(path, stack, expand_map);
                let mode_val = self.gen_expr(mode, stack, expand_map);
                let fn_type = self.context.i64_type().fn_type(&[
                    self.context.i64_type().into(),
                    self.context.i64_type().into(),
                ], false);
                let func = self.get_or_add_external_fn("llm_file_open", fn_type);
                let call = self.builder.build_call(func, &[path_val.into(), mode_val.into()], "file_open").unwrap();
                let res = self.get_call_res(call);
                self.maybe_drop_val(path, path_val, stack);
                self.maybe_drop_val(mode, mode_val, stack);
                res
            }
            Expr::Seq(e1, e2) => {
                let v1 = self.gen_expr(e1, stack, expand_map);
                self.maybe_drop_val(e1, v1, stack);
                self.gen_expr(e2, stack, expand_map)
            }
            Expr::Pack(e) => {
                let val = self.gen_expr(e, stack, expand_map);
                let shape_name = self.infer_shape(e, stack).expect("E006");
                let shapes = self.shapes.borrow();
                let fields = shapes.get(&shape_name).expect("E006");
                let fields_csv = fields.join(",");
                let val_int = self.as_int(val);
                let fields_val = self.gen_string_constant(&fields_csv);
                let fields_int = self.as_int(fields_val);
                let fn_type = self.context.i64_type().fn_type(&[self.context.i64_type().into(), self.context.i64_type().into()], false);
                let func = self.get_or_add_external_fn("llm_pack", fn_type);
                let call = self.builder.build_call(func, &[val_int.into(), fields_int.into()], "json").unwrap();
                let res = self.get_call_res(call);
                self.maybe_drop_val(e, val, stack);
                res
            }
            Expr::Unpack(e, shape_name) => {
                let json_val = self.gen_expr(e, stack, expand_map);
                let shapes = self.shapes.borrow();
                let fields = shapes.get(shape_name).expect("E006");
                let fields_csv = fields.join(",");
                let fields_val = self.gen_string_constant(&fields_csv);
                let fields_int = self.as_int(fields_val);
                let fn_type = self.context.i64_type().fn_type(&[self.context.i64_type().into(), self.context.i64_type().into()], false);
                let func = self.get_or_add_external_fn("llm_unpack", fn_type);
                let call = self.builder.build_call(func, &[json_val.into(), fields_int.into()], "inst").unwrap();
                let res = self.get_call_res(call);
                self.maybe_drop_val(e, json_val, stack);
                res
            }
            Expr::Map(inst_expr, field_name, func_expr) => {
                let inst_ptr_val = self.gen_expr(inst_expr, stack, expand_map);
                let inst_ptr_int = self.as_int(inst_ptr_val);
                let inst_ptr = self.builder.build_int_to_ptr(inst_ptr_int, self.context.ptr_type(inkwell::AddressSpace::default()), "inst_ptr").unwrap();
                let count_val = self.builder.build_load(self.context.i64_type(), inst_ptr, "count").unwrap();
                let count = self.as_int(count_val);
                let shape_name = self.infer_shape(inst_expr, stack).expect("E006");
                let shapes = self.shapes.borrow();
                let fields = shapes.get(&shape_name).expect("E006");
                let mut members: Vec<BasicValueEnum<'ctx>> = Vec::new();
                members.push(count.into());
                for _field_type_name in fields {
                    let size_bytes = self.builder.build_int_mul(count, self.context.i64_type().const_int(8, false), "size").unwrap();
                    let fn_type = self.context.ptr_type(inkwell::AddressSpace::default()).fn_type(&[self.context.i64_type().into()], false);
                    let func = self.get_or_add_external_fn("llm_alloc", fn_type);
                    let call = self.builder.build_call(func, &[size_bytes.into()], "col_ptr_raw").unwrap();
                    let ptr_val = self.get_call_res(call);
                    let ptr = self.builder.build_ptr_to_int(ptr_val.into_pointer_value(), self.context.i64_type(), "col_ptr").unwrap();
                    members.push(ptr.into());
                }
                let struct_size = (members.len() as u64) * 8;
                let fn_type = self.context.ptr_type(inkwell::AddressSpace::default()).fn_type(&[self.context.i64_type().into()], false);
                let func = self.get_or_add_external_fn("llm_alloc", fn_type);
                let call = self.builder.build_call(func, &[self.context.i64_type().const_int(struct_size, false).into()], "struct_ptr_raw").unwrap();
                let struct_ptr_raw = self.get_call_res(call).into_pointer_value();
                let new_inst_ptr_int = self.builder.build_ptr_to_int(struct_ptr_raw, self.context.i64_type(), "new_inst_ptr").unwrap();
                for (i, val) in members.into_iter().enumerate() {
                    let member_ptr = unsafe { self.builder.build_gep(self.context.i64_type(), struct_ptr_raw, &[self.context.i64_type().const_int(i as u64, false)], "member_ptr").unwrap() };
                    self.builder.build_store(member_ptr, val).unwrap();
                }
                let parent_func = self.builder.get_insert_block().unwrap().get_parent().unwrap();
                let loop_bb = self.context.append_basic_block(parent_func, "map_loop");
                let after_bb = self.context.append_basic_block(parent_func, "after_map");
                let i_ptr = self.builder.build_alloca(self.context.i64_type(), "i").unwrap();
                self.builder.build_store(i_ptr, self.context.i64_type().const_int(0, false)).unwrap();
                self.builder.build_unconditional_branch(loop_bb).unwrap();
                self.builder.position_at_end(loop_bb);
                let i_val = self.builder.build_load(self.context.i64_type(), i_ptr, "i_val").unwrap();
                let i = self.as_int(i_val);
                let cond = self.builder.build_int_compare(inkwell::IntPredicate::SLT, i, count, "loopcond").unwrap();
                let loop_body_bb = self.context.append_basic_block(parent_func, "map_body");
                self.builder.build_conditional_branch(cond, loop_body_bb, after_bb).unwrap();
                self.builder.position_at_end(loop_body_bb);
                let mut field_idx = 0;
                for (idx, f) in fields.iter().enumerate() { if f == field_name { field_idx = idx + 1; break; } }
                let col_ptr_ptr = unsafe { self.builder.build_gep(self.context.i64_type(), inst_ptr, &[self.context.i64_type().const_int(field_idx as u64, false)], "col_ptr_ptr").unwrap() };
                let col_ptr_int_val = self.builder.build_load(self.context.i64_type(), col_ptr_ptr, "col_ptr_int").unwrap();
                let col_ptr_int = self.as_int(col_ptr_int_val);
                let col_ptr = self.builder.build_int_to_ptr(col_ptr_int, self.context.ptr_type(inkwell::AddressSpace::default()), "col_ptr").unwrap();
                let old_val = self.builder.build_load(self.context.i64_type(), unsafe { self.builder.build_gep(self.context.i64_type(), col_ptr, &[i], "gep").unwrap() }, "old_val").unwrap();
                let res_val = if let Expr::Identifier(ref name) = **func_expr {
                    let resolved = self.resolve_func_name(name);
                    let function = self.module.get_function(&resolved).expect("E010");
                    self.get_call_res(self.builder.build_call(function, &[old_val.into()], "mapped").unwrap())
                } else {
                    old_val
                };
                let new_col_ptr_ptr = unsafe { self.builder.build_gep(self.context.i64_type(), struct_ptr_raw, &[self.context.i64_type().const_int(field_idx as u64, false)], "new_col_ptr_ptr").unwrap() };
                let new_col_ptr_int_val = self.builder.build_load(self.context.i64_type(), new_col_ptr_ptr, "new_col_ptr_int").unwrap();
                let new_col_ptr_int = self.as_int(new_col_ptr_int_val);
                let new_col_ptr = self.builder.build_int_to_ptr(new_col_ptr_int, self.context.ptr_type(inkwell::AddressSpace::default()), "new_col_ptr").unwrap();
                self.builder.build_store(unsafe { self.builder.build_gep(self.context.i64_type(), new_col_ptr, &[i], "gep").unwrap() }, res_val).unwrap();
                for (idx, _) in fields.iter().enumerate() {
                    let current_idx = idx + 1;
                    if current_idx != field_idx {
                        let src_col_ptr_ptr = unsafe { self.builder.build_gep(self.context.i64_type(), inst_ptr, &[self.context.i64_type().const_int(current_idx as u64, false)], "src_col").unwrap() };
                        let src_col_int_val = self.builder.build_load(self.context.i64_type(), src_col_ptr_ptr, "src_val").unwrap();
                        let src_col_int = self.as_int(src_col_int_val);
                        let src_col = self.builder.build_int_to_ptr(src_col_int, self.context.ptr_type(inkwell::AddressSpace::default()), "src_ptr").unwrap();
                        let val = self.builder.build_load(self.context.i64_type(), unsafe { self.builder.build_gep(self.context.i64_type(), src_col, &[i], "gep").unwrap() }, "v").unwrap();
                        let dst_col_ptr_ptr = unsafe { self.builder.build_gep(self.context.i64_type(), struct_ptr_raw, &[self.context.i64_type().const_int(current_idx as u64, false)], "dst_col").unwrap() };
                        let dst_col_int_val = self.builder.build_load(self.context.i64_type(), dst_col_ptr_ptr, "dst_val").unwrap();
                        let dst_col_int = self.as_int(dst_col_int_val);
                        let dst_col = self.builder.build_int_to_ptr(dst_col_int, self.context.ptr_type(inkwell::AddressSpace::default()), "dst_ptr").unwrap();
                        self.builder.build_store(unsafe { self.builder.build_gep(self.context.i64_type(), dst_col, &[i], "gep").unwrap() }, val).unwrap();
                    }
                }
                self.builder.build_store(i_ptr, self.builder.build_int_add(i, self.context.i64_type().const_int(1, false), "next_i").unwrap()).unwrap();
                self.builder.build_unconditional_branch(loop_bb).unwrap();
                self.builder.position_at_end(after_bb);
                self.maybe_drop_val(inst_expr, inst_ptr_val, stack);
                new_inst_ptr_int.into()
            }
            Expr::Filter(inst_expr, func_expr) => {
                let inst_ptr_val = self.gen_expr(inst_expr, stack, expand_map);
                let inst_ptr_int = self.as_int(inst_ptr_val);
                let inst_ptr = self.builder.build_int_to_ptr(inst_ptr_int, self.context.ptr_type(inkwell::AddressSpace::default()), "inst_ptr").unwrap();
                let count_val = self.builder.build_load(self.context.i64_type(), inst_ptr, "count").unwrap();
                let count = self.as_int(count_val);
                let shape_name = self.infer_shape(inst_expr, stack).expect("E006");
                let shapes = self.shapes.borrow();
                let fields = shapes.get(&shape_name).expect("E006");
                let parent_func = self.builder.get_insert_block().unwrap().get_parent().unwrap();
                let count_loop_bb = self.context.append_basic_block(parent_func, "filter_count_loop");
                let count_after_bb = self.context.append_basic_block(parent_func, "after_filter_count");
                let matching_count_ptr = self.builder.build_alloca(self.context.i64_type(), "matching_count").unwrap();
                self.builder.build_store(matching_count_ptr, self.context.i64_type().const_int(0, false)).unwrap();
                let i_ptr = self.builder.build_alloca(self.context.i64_type(), "i").unwrap();
                self.builder.build_store(i_ptr, self.context.i64_type().const_int(0, false)).unwrap();
                self.builder.build_unconditional_branch(count_loop_bb).unwrap();
                self.builder.position_at_end(count_loop_bb);
                let i_val = self.builder.build_load(self.context.i64_type(), i_ptr, "i_val").unwrap();
                let i = self.as_int(i_val);
                let cond = self.builder.build_int_compare(inkwell::IntPredicate::SLT, i, count, "loopcond").unwrap();
                let count_body_bb = self.context.append_basic_block(parent_func, "filter_count_body");
                self.builder.build_conditional_branch(cond, count_body_bb, count_after_bb).unwrap();
                self.builder.position_at_end(count_body_bb);
                let mut row_vals: Vec<BasicValueEnum<'ctx>> = Vec::new();
                for (idx, field_type_name) in fields.iter().enumerate() {
                    let col_ptr_ptr = unsafe { self.builder.build_gep(self.context.i64_type(), inst_ptr, &[self.context.i64_type().const_int((idx + 1) as u64, false)], "col_ptr_ptr").unwrap() };
                    let col_ptr_int_val = self.builder.build_load(self.context.i64_type(), col_ptr_ptr, "col_ptr_int").unwrap();
                    let col_ptr_int = self.as_int(col_ptr_int_val);
                    let col_ptr = self.builder.build_int_to_ptr(col_ptr_int, self.context.ptr_type(inkwell::AddressSpace::default()), "col_ptr").unwrap();
                    let llvm_type = self.get_llvm_type(field_type_name);
                    let val = self.builder.build_load(llvm_type, unsafe { self.builder.build_gep(llvm_type, col_ptr, &[i], "gep").unwrap() }, "v").unwrap();
                    row_vals.push(val);
                }
                let matched = if let Expr::Identifier(ref name) = **func_expr {
                    let resolved = self.resolve_func_name(name);
                    let function = self.module.get_function(&resolved).expect("E010");
                    let mut meta_vals = Vec::new();
                    for v in &row_vals { meta_vals.push((*v).into()); }
                    let res_val = self.get_call_res(self.builder.build_call(function, &meta_vals, "pred").unwrap());
                    let res = self.as_int(res_val);
                    self.builder.build_int_compare(inkwell::IntPredicate::NE, res, self.context.i64_type().const_int(0, false), "is_matched").unwrap()
                } else {
                    self.context.bool_type().const_int(1, false)
                };
                let cur_matching_val = self.builder.build_load(self.context.i64_type(), matching_count_ptr, "c").unwrap();
                let cur_matching = self.as_int(cur_matching_val);
                let inc = self.builder.build_int_z_extend(matched, self.context.i64_type(), "inc").unwrap();
                self.builder.build_store(matching_count_ptr, self.builder.build_int_add(cur_matching, inc, "new_c").unwrap()).unwrap();
                self.builder.build_store(i_ptr, self.builder.build_int_add(i, self.context.i64_type().const_int(1, false), "next_i").unwrap()).unwrap();
                self.builder.build_unconditional_branch(count_loop_bb).unwrap();
                self.builder.position_at_end(count_after_bb);
                let final_matching_count_val = self.builder.build_load(self.context.i64_type(), matching_count_ptr, "final_c").unwrap();
                let final_matching_count = self.as_int(final_matching_count_val);
                let mut members: Vec<BasicValueEnum<'ctx>> = Vec::new();
                members.push(final_matching_count.into());
                for _field_type_name in fields {
                    let size_bytes = self.builder.build_int_mul(final_matching_count, self.context.i64_type().const_int(8, false), "size").unwrap();
                    let fn_type = self.context.ptr_type(inkwell::AddressSpace::default()).fn_type(&[self.context.i64_type().into()], false);
                    let func = self.get_or_add_external_fn("llm_alloc", fn_type);
                    let call = self.builder.build_call(func, &[size_bytes.into()], "col_ptr_raw").unwrap();
                    let ptr_val = self.get_call_res(call);
                    let ptr = self.builder.build_ptr_to_int(ptr_val.into_pointer_value(), self.context.i64_type(), "col_ptr").unwrap();
                    members.push(ptr.into());
                }
                let struct_size = (members.len() as u64) * 8;
                let fn_type = self.context.ptr_type(inkwell::AddressSpace::default()).fn_type(&[self.context.i64_type().into()], false);
                let func = self.get_or_add_external_fn("llm_alloc", fn_type);
                let call = self.builder.build_call(func, &[self.context.i64_type().const_int(struct_size, false).into()], "struct_ptr_raw").unwrap();
                let struct_ptr_raw = self.get_call_res(call).into_pointer_value();
                let new_inst_ptr_int = self.builder.build_ptr_to_int(struct_ptr_raw, self.context.i64_type(), "new_inst_ptr").unwrap();
                for (idx, val) in members.into_iter().enumerate() {
                    let member_ptr = unsafe { self.builder.build_gep(self.context.i64_type(), struct_ptr_raw, &[self.context.i64_type().const_int(idx as u64, false)], "member_ptr").unwrap() };
                    self.builder.build_store(member_ptr, val).unwrap();
                }
                let copy_loop_bb = self.context.append_basic_block(parent_func, "filter_copy_loop");
                let copy_after_bb = self.context.append_basic_block(parent_func, "after_filter_copy");
                let next_dst_idx_ptr = self.builder.build_alloca(self.context.i64_type(), "dst_idx").unwrap();
                self.builder.build_store(next_dst_idx_ptr, self.context.i64_type().const_int(0, false)).unwrap();
                self.builder.build_store(i_ptr, self.context.i64_type().const_int(0, false)).unwrap();
                self.builder.build_unconditional_branch(copy_loop_bb).unwrap();
                self.builder.position_at_end(copy_loop_bb);
                let i_val2 = self.builder.build_load(self.context.i64_type(), i_ptr, "i_val").unwrap();
                let i2 = self.as_int(i_val2);
                let cond2 = self.builder.build_int_compare(inkwell::IntPredicate::SLT, i2, count, "loopcond").unwrap();
                let copy_body_bb = self.context.append_basic_block(parent_func, "filter_copy_body");
                self.builder.build_conditional_branch(cond2, copy_body_bb, copy_after_bb).unwrap();
                self.builder.position_at_end(copy_body_bb);
                let mut row_vals2: Vec<BasicValueEnum<'ctx>> = Vec::new();
                for (idx, field_type_name) in fields.iter().enumerate() {
                    let col_ptr_ptr = unsafe { self.builder.build_gep(self.context.i64_type(), inst_ptr, &[self.context.i64_type().const_int((idx + 1) as u64, false)], "col_ptr_ptr").unwrap() };
                    let col_ptr_int_val = self.builder.build_load(self.context.i64_type(), col_ptr_ptr, "col_ptr_int").unwrap();
                    let col_ptr_int = self.as_int(col_ptr_int_val);
                    let col_ptr = self.builder.build_int_to_ptr(col_ptr_int, self.context.ptr_type(inkwell::AddressSpace::default()), "col_ptr").unwrap();
                    let llvm_type = self.get_llvm_type(field_type_name);
                    let val = self.builder.build_load(llvm_type, unsafe { self.builder.build_gep(llvm_type, col_ptr, &[i2], "gep").unwrap() }, "v").unwrap();
                    row_vals2.push(val);
                }
                let matched2 = if let Expr::Identifier(ref name) = **func_expr {
                    let resolved = self.resolve_func_name(name);
                    let function = self.module.get_function(&resolved).expect("E010");
                    let mut meta_vals = Vec::new();
                    for v in &row_vals2 { meta_vals.push((*v).into()); }
                    let res_val = self.get_call_res(self.builder.build_call(function, &meta_vals, "pred").unwrap());
                    let res = self.as_int(res_val);
                    self.builder.build_int_compare(inkwell::IntPredicate::NE, res, self.context.i64_type().const_int(0, false), "is_matched").unwrap()
                } else {
                    self.context.bool_type().const_int(1, false)
                };
                let then_copy_bb = self.context.append_basic_block(parent_func, "then_copy");
                let end_copy_bb = self.context.append_basic_block(parent_func, "end_copy");
                self.builder.build_conditional_branch(matched2, then_copy_bb, end_copy_bb).unwrap();
                self.builder.position_at_end(then_copy_bb);
                let dst_idx_val = self.builder.build_load(self.context.i64_type(), next_dst_idx_ptr, "d").unwrap();
                let dst_idx = self.as_int(dst_idx_val);
                for (idx, _) in fields.iter().enumerate() {
                    let dst_col_ptr_ptr = unsafe { self.builder.build_gep(self.context.i64_type(), struct_ptr_raw, &[self.context.i64_type().const_int((idx + 1) as u64, false)], "dst_col").unwrap() };
                    let dst_col_int_val = self.builder.build_load(self.context.i64_type(), dst_col_ptr_ptr, "dst_val").unwrap();
                    let dst_col_int = self.as_int(dst_col_int_val);
                    let dst_col = self.builder.build_int_to_ptr(dst_col_int, self.context.ptr_type(inkwell::AddressSpace::default()), "dst_ptr").unwrap();
                    let llvm_type = self.get_llvm_type(&fields[idx]);
                    self.builder.build_store(unsafe { self.builder.build_gep(llvm_type, dst_col, &[dst_idx], "gep").unwrap() }, row_vals2[idx]).unwrap();
                }
                self.builder.build_store(next_dst_idx_ptr, self.builder.build_int_add(dst_idx, self.context.i64_type().const_int(1, false), "next_d").unwrap()).unwrap();
                self.builder.build_unconditional_branch(end_copy_bb).unwrap();
                self.builder.position_at_end(end_copy_bb);
                self.builder.build_store(i_ptr, self.builder.build_int_add(i2, self.context.i64_type().const_int(1, false), "next_i").unwrap()).unwrap();
                self.builder.build_unconditional_branch(copy_loop_bb).unwrap();
                self.builder.position_at_end(copy_after_bb);
                self.maybe_drop_val(inst_expr, inst_ptr_val, stack);
                new_inst_ptr_int.into()
            }
            Expr::MoneyOp(op, left, right) => {
                let lhs_val = self.gen_expr(left, stack, expand_map);
                let rhs_val = self.gen_expr(right, stack, expand_map);
                let lhs = self.as_int(lhs_val);
                let rhs = self.as_int(rhs_val);
                let scale = self.context.i64_type().const_int(10000, false);
                match op {
                    Token::Add => self.builder.build_int_add(lhs, rhs, "money_add").unwrap().into(),
                    Token::Sub => self.builder.build_int_sub(lhs, rhs, "money_sub").unwrap().into(),
                    Token::Mul => {
                        let mul = self.builder.build_int_mul(lhs, rhs, "mul_raw").unwrap();
                        self.builder.build_int_signed_div(mul, scale, "money_mul").unwrap().into()
                    }
                    Token::Div => {
                        let scaled_lhs = self.builder.build_int_mul(lhs, scale, "lhs_scaled").unwrap();
                        self.builder.build_int_signed_div(scaled_lhs, rhs, "money_div").unwrap().into()
                    }
                    _ => panic!("E008"),
                }
            }
            Expr::MoneyStr(e) => {
                let val = self.gen_expr(e, stack, expand_map);
                let fn_type = self.context.i64_type().fn_type(&[self.context.i64_type().into()], false);
                let func = self.get_or_add_external_fn("llm_money_format", fn_type);
                let call = self.builder.build_call(func, &[val.into()], "mstr").unwrap();
                self.get_call_res(call)
            }
            Expr::TimeOp(op, left, right) => {
                let lhs_val = self.gen_expr(left, stack, expand_map);
                let rhs_val = self.gen_expr(right, stack, expand_map);
                let lhs = self.as_int(lhs_val);
                let rhs = self.as_int(rhs_val);
                match op {
                    Token::Add => self.builder.build_int_add(lhs, rhs, "time_add").unwrap().into(),
                    Token::Sub => self.builder.build_int_sub(lhs, rhs, "time_sub").unwrap().into(),
                    _ => panic!("E008"),
                }
            }
            Expr::TimeZone => {
                let fn_type = self.context.i64_type().fn_type(&[], false);
                let func = self.get_or_add_external_fn("llm_timezone", fn_type);
                let call = self.builder.build_call(func, &[], "tz").unwrap();
                self.get_call_res(call)
            }
            Expr::Panic(e) => {
                let msg = self.gen_expr(e, stack, expand_map);
                let fn_type = self.context.void_type().fn_type(&[self.context.i64_type().into()], false);
                let func = self.get_or_add_external_fn("llm_panic", fn_type);
                self.builder.build_call(func, &[msg.into()], "").unwrap();
                self.context.i64_type().const_int(0, false).into()
            }
            Expr::Trap(try_expr, fallback_expr) => {
                let mut captures = Vec::new();
                for (i, item) in stack.iter().enumerate() {
                    if item.state == VariableState::Available || item.state == VariableState::Borrowed {
                        captures.push((i, item.value, item.shape.clone(), item.is_ptr));
                    }
                }

                let trap_id = self.module.get_functions().count();
                let try_name = format!("trap_try_{}", trap_id);
                let fallback_name = format!("trap_fallback_{}", trap_id);

                let try_fn = self.gen_trap_sub_fn(try_expr, stack, expand_map, &captures, &try_name);
                let fallback_fn = self.gen_trap_sub_fn(fallback_expr, stack, expand_map, &captures, &fallback_name);

                let env_alloc = self.builder.build_array_alloca(self.context.i64_type(), self.context.i64_type().const_int(captures.len() as u64, false), "env").unwrap();
                for (i, (_orig_idx, val, _shape, _is_ptr)) in captures.iter().enumerate() {
                    let member_ptr = unsafe { self.builder.build_gep(self.context.i64_type(), env_alloc, &[self.context.i64_type().const_int(i as u64, false)], "cap_store").unwrap() };
                    self.builder.build_store(member_ptr, *val).unwrap();
                }

                let i64_type = self.context.i64_type();
                let try_fn_type = i64_type.fn_type(&[i64_type.into(), i64_type.into(), i64_type.into(), i64_type.into()], false);
                let try_func = self.get_or_add_external_fn("llm_try", try_fn_type);
                
                let try_fn_int = self.builder.build_ptr_to_int(try_fn.as_global_value().as_pointer_value(), i64_type, "try_ptr").unwrap();
                let fallback_fn_int = self.builder.build_ptr_to_int(fallback_fn.as_global_value().as_pointer_value(), i64_type, "fallback_ptr").unwrap();
                let env_int = self.builder.build_ptr_to_int(env_alloc, i64_type, "env_ptr").unwrap();
                
                let call = self.builder.build_call(try_func, &[try_fn_int.into(), env_int.into(), fallback_fn_int.into(), env_int.into()], "try_res").unwrap();
                self.get_call_res(call)
            }
            Expr::Shape(_, _, _) | Expr::Import(..) | Expr::Define(_, _, _, _) => {
                self.context.i64_type().const_int(0, false).into()
            }
        }
    }

    fn is_owned_ptr(&self, expr: &Expr, stack: &[StackItem<'ctx>]) -> bool {
        let stack_ptrs: Vec<bool> = stack.iter().map(|item| item.is_ptr).collect();
        if !expr.returns_ptr_with_stack(&stack_ptrs, &self.fn_returns_ptr.borrow()) {
            return false;
        }
        match expr {
            Expr::Borrow(_) | Expr::MutBorrow(_) => false,
            Expr::DeBruijn(_) => false,
            Expr::Expand(_) => false,
            Expr::Move(_) => true,
            _ => true,
        }
    }

    fn maybe_drop_val(&self, expr: &Expr, val: BasicValueEnum<'ctx>, stack: &[StackItem<'ctx>]) {
        if self.is_owned_ptr(expr, stack) {
            let shape = self.infer_shape(expr, stack);
            self.emit_auto_drop(val, shape.as_deref(), true);
        }
    }
}

