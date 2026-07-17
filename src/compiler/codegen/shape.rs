use crate::compiler::ast::{infer_shape_from_stack, Expr};
use crate::compiler::codegen::{CodeGen, StackItem};

impl<'ctx> CodeGen<'ctx> {
    pub fn gen_shape(&self, name: &str, fields: &[String], exported: bool) {
        if exported {
            self.has_exports.set(true);
            self.exports.borrow_mut().push(name.to_string());
        }
        self.shapes.borrow_mut().insert(name.to_string(), fields.to_vec());
    }

    pub fn infer_shape(&self, expr: &Expr, stack: &[StackItem<'ctx>]) -> Option<String> {
        let stack_shapes: Vec<Option<String>> = stack.iter().map(|item| item.shape.clone()).collect();
        infer_shape_from_stack(expr, &stack_shapes)
    }
}
