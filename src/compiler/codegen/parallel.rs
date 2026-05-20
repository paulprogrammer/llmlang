use crate::compiler::ast::Expr;
use crate::compiler::codegen::{CodeGen, StackItem, VariableState};
use inkwell::values::{FunctionValue, BasicValueEnum};
use std::collections::HashMap;

impl<'ctx> CodeGen<'ctx> {
    pub fn gen_parallel_expr(&self, expr: &Expr, stack: &mut Vec<StackItem<'ctx>>, expand_map: &HashMap<String, usize>) -> BasicValueEnum<'ctx> {
        let mut captures = Vec::new();
        for (i, item) in stack.iter().enumerate() {
            if item.state == VariableState::Available || item.state == VariableState::Borrowed {
                captures.push((i, item.value, item.shape.clone(), item.is_ptr));
            }
        }

        let i64_ptr = self.context.ptr_type(inkwell::AddressSpace::default());
        let task_fn_type = self.context.i64_type().fn_type(&[i64_ptr.into()], false);
        let task_id = self.module.get_functions().count();
        let task_name = format!("parallel_task_{}", task_id);
        let task_fn = self.module.add_function(&task_name, task_fn_type, None);
        let entry = self.context.append_basic_block(task_fn, "entry");
        
        let current_bb = self.builder.get_insert_block().unwrap();
        self.builder.position_at_end(entry);

        let env_ptr = task_fn.get_nth_param(0).unwrap().into_pointer_value();
        let mut task_stack = Vec::new();
        for (i, (_orig_idx, _val, shape, is_ptr)) in captures.iter().enumerate() {
            let member_ptr = unsafe { self.builder.build_gep(self.context.i64_type(), env_ptr, &[self.context.i64_type().const_int(i as u64, false)], "cap").unwrap() };
            let loaded = self.builder.build_load(self.context.i64_type(), member_ptr, "val").unwrap();
            task_stack.push(StackItem { value: loaded, state: VariableState::Borrowed, shape: shape.clone(), is_ptr: *is_ptr });
        }

        let res = self.gen_expr(expr, &mut task_stack, expand_map);
        self.builder.build_return(Some(&res)).unwrap();
        self.builder.position_at_end(current_bb);

        let env_alloc = self.builder.build_array_alloca(self.context.i64_type(), self.context.i64_type().const_int(captures.len() as u64, false), "env").unwrap();
        for (i, (_orig_idx, val, _shape, _is_ptr)) in captures.iter().enumerate() {
            let member_ptr = unsafe { self.builder.build_gep(self.context.i64_type(), env_alloc, &[self.context.i64_type().const_int(i as u64, false)], "cap_store").unwrap() };
            self.builder.build_store(member_ptr, *val).unwrap();
        }

        let fork_fn_type = self.context.i64_type().fn_type(&[self.context.i64_type().into(), self.context.i64_type().into()], false);
        let fork_fn = self.get_or_add_external_fn("llm_fork", fork_fn_type);
        let fn_int = self.builder.build_ptr_to_int(task_fn.as_global_value().as_pointer_value(), self.context.i64_type(), "fn_ptr").unwrap();
        let env_int = self.builder.build_ptr_to_int(env_alloc, self.context.i64_type(), "env_ptr").unwrap();
        let call = self.builder.build_call(fork_fn, &[fn_int.into(), env_int.into()], "handle").unwrap();
        self.get_call_res(call)
    }

    pub fn gen_trap_sub_fn(&self, expr: &Expr, _stack: &mut Vec<StackItem<'ctx>>, expand_map: &HashMap<String, usize>, captures: &[(usize, BasicValueEnum<'ctx>, Option<String>, bool)], name: &str) -> FunctionValue<'ctx> {
        let i64_ptr = self.context.ptr_type(inkwell::AddressSpace::default());
        let task_fn_type = self.context.i64_type().fn_type(&[i64_ptr.into()], false);
        let task_fn = self.module.add_function(name, task_fn_type, None);
        let entry = self.context.append_basic_block(task_fn, "entry");
        
        let current_bb = self.builder.get_insert_block().unwrap();
        self.builder.position_at_end(entry);

        let env_ptr = task_fn.get_nth_param(0).unwrap().into_pointer_value();
        let mut task_stack = Vec::new();
        for (i, (_orig_idx, _val, shape, is_ptr)) in captures.iter().enumerate() {
            let member_ptr = unsafe { self.builder.build_gep(self.context.i64_type(), env_ptr, &[self.context.i64_type().const_int(i as u64, false)], "cap").unwrap() };
            let loaded = self.builder.build_load(self.context.i64_type(), member_ptr, "val").unwrap();
            task_stack.push(StackItem { value: loaded, state: VariableState::Borrowed, shape: shape.clone(), is_ptr: *is_ptr });
        }

        let res = self.gen_expr(expr, &mut task_stack, expand_map);
        for item in task_stack {
            if item.state == VariableState::Available {
                self.emit_auto_drop(item.value, item.shape.as_deref(), item.is_ptr);
            }
        }
        self.builder.build_return(Some(&res)).unwrap();
        self.builder.position_at_end(current_bb);
        task_fn
    }
}
