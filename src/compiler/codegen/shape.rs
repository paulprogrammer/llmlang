use crate::compiler::ast::Expr;
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
        match expr {
            Expr::New(name, _) => Some(name.clone()),
            Expr::Unpack(_, name) => Some(name.clone()),
            Expr::Map(e, _, _) => self.infer_shape(e, stack),
            Expr::Filter(e, _) => self.infer_shape(e, stack),
            Expr::DeBruijn(idx) => {
                if *idx < stack.len() {
                    stack[stack.len() - 1 - idx].shape.clone()
                } else {
                    None
                }
            }
            Expr::Move(inner) | Expr::Borrow(inner) | Expr::MutBorrow(inner) => self.infer_shape(inner, stack),
            _ => None,
        }
    }
}
