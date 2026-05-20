use std::collections::HashMap;
use crate::compiler::ast::{Expr};
use crate::compiler::codegen::VariableState;
use crate::compiler::lexer::Token;

pub struct VerificationContext {
    pub shapes: HashMap<String, Vec<String>>,
    pub functions: HashMap<String, usize>, // name -> arity
    pub stack: Vec<VariableState>,
    pub stack_shapes: Vec<Option<String>>,
    pub expand_map: HashMap<String, usize>,
}

fn infer_shape(expr: &Expr, stack_shapes: &[Option<String>]) -> Option<String> {
    match expr {
        Expr::New(name, _) => Some(name.clone()),
        Expr::Unpack(_, name) => Some(name.clone()),
        Expr::Map(e, _, _) => infer_shape(e, stack_shapes),
        Expr::Filter(e, _) => infer_shape(e, stack_shapes),
        Expr::DeBruijn(idx) => {
            if *idx < stack_shapes.len() {
                stack_shapes[stack_shapes.len() - 1 - idx].clone()
            } else {
                None
            }
        }
        Expr::Move(inner) | Expr::Borrow(inner) | Expr::MutBorrow(inner) => infer_shape(inner, stack_shapes),
        _ => None,
    }
}

use crate::compiler::error::CompileError;

pub fn verify_module(exprs: &[Expr], filename: &str) -> Result<(), CompileError> {
    let mut shapes = HashMap::new();
    let mut functions = HashMap::new();

    // First pass: register shapes, functions, and imports
    for expr in exprs {
        match expr {
            Expr::Shape(name, fields, _) => {
                shapes.insert(name.clone(), fields.clone());
            }
            Expr::Import(_, symbol, arity) => {
                functions.insert(symbol.clone(), *arity);
            }
            Expr::Define(name, params, _, _) => {
                functions.insert(name.clone(), params.len());
            }
            _ => {}
        }
    }

    // Second pass: verify all function bodies
    for expr in exprs {
        if let Expr::Define(_name, params, body, _) = expr {
            let mut stack = Vec::new();
            let mut stack_shapes = Vec::new();
            let mut expand_map = HashMap::new();

            for (i, param) in params.iter().enumerate() {
                stack.push(VariableState::Available);
                stack_shapes.push(None);
                if param.expand {
                    expand_map.insert(param.name.clone(), i);
                }
            }

            let mut context = VerificationContext {
                shapes: shapes.clone(),
                functions: functions.clone(),
                stack,
                stack_shapes,
                expand_map,
            };

            verify_expr(body, &mut context).map_err(|err_code| {
                CompileError::new(&err_code, filename, 1)
            })?;
        }
    }

    Ok(())
}

pub fn verify_expr(expr: &Expr, context: &mut VerificationContext) -> Result<(), String> {
    match expr {
        Expr::Integer(_) | Expr::Float(_) | Expr::String(_) | Expr::TimeZone | Expr::TimeNow | Expr::TimeNano => Ok(()),
        Expr::DeBruijn(index) => {
            if *index >= context.stack.len() {
                return Err("E003".to_string());
            }
            let actual_idx = context.stack.len() - 1 - index;
            if context.stack[actual_idx] == VariableState::Moved {
                return Err("E004".to_string());
            }
            Ok(())
        }
        Expr::Identifier(name) => {
            if context.functions.contains_key(name) {
                Ok(())
            } else {
                Err(format!("E013: Unknown identifier {}", name))
            }
        }
        Expr::Expand(name) => {
            if let Some(&index) = context.expand_map.get(name) {
                if index >= context.stack.len() {
                    return Err("E003".to_string());
                }
                if context.stack[index] == VariableState::Moved {
                    return Err("E004".to_string());
                }
                Ok(())
            } else {
                Err(format!("E013: Unknown identifier {}", name))
            }
        }
        Expr::Move(inner) => {
            if let Expr::DeBruijn(index) = &**inner {
                if *index >= context.stack.len() {
                    return Err("E003".to_string());
                }
                let actual_idx = context.stack.len() - 1 - index;
                if context.stack[actual_idx] == VariableState::Moved {
                    return Err("E005".to_string());
                }
                if context.stack[actual_idx] == VariableState::Borrowed {
                    return Err("E016".to_string());
                }
                context.stack[actual_idx] = VariableState::Moved;
                Ok(())
            } else if let Expr::Expand(name) = &**inner {
                if let Some(&index) = context.expand_map.get(name) {
                    if index >= context.stack.len() {
                        return Err("E003".to_string());
                    }
                    if context.stack[index] == VariableState::Moved {
                        return Err("E005".to_string());
                    }
                    if context.stack[index] == VariableState::Borrowed {
                        return Err("E016".to_string());
                    }
                    context.stack[index] = VariableState::Moved;
                    Ok(())
                } else {
                    Err(format!("E013: Unknown identifier {}", name))
                }
            } else {
                verify_expr(inner, context)
            }
        }
        Expr::Borrow(inner) | Expr::MutBorrow(inner) => {
            verify_expr(inner, context)
        }
        Expr::Let(_, val_expr, body_expr) => {
            verify_expr(val_expr, context)?;
            let val_shape = infer_shape(val_expr, &context.stack_shapes);
            context.stack.push(VariableState::Available);
            context.stack_shapes.push(val_shape);
            let res = verify_expr(body_expr, context);
            context.stack.pop();
            context.stack_shapes.pop();
            res
        }
        Expr::If(cond, then_branch, else_branch) => {
            verify_expr(cond, context)?;
            let initial_states = context.stack.clone();
            let initial_shapes = context.stack_shapes.clone();

            verify_expr(then_branch, context)?;
            let then_states = context.stack.clone();

            context.stack = initial_states;
            context.stack_shapes = initial_shapes;

            verify_expr(else_branch, context)?;
            let else_states = context.stack.clone();

            if then_states != else_states {
                return Err("E009".to_string());
            }
            Ok(())
        }
        Expr::Seq(e1, e2) => {
            verify_expr(e1, context)?;
            verify_expr(e2, context)
        }
        Expr::New(shape_name, count_expr) => {
            if !context.shapes.contains_key(shape_name) {
                return Err("E006".to_string());
            }
            verify_expr(count_expr, context)
        }
        Expr::Get(instance, field, index) => {
            verify_expr(instance, context)?;
            verify_expr(index, context)?;
            let inferred = infer_shape(instance, &context.stack_shapes);
            let mut found = false;
            if let Some(ref shape_name) = inferred {
                if let Some(fields) = context.shapes.get(shape_name) {
                    if fields.iter().any(|f| f == field) {
                        found = true;
                    }
                }
            } else {
                for fields in context.shapes.values() {
                    if fields.iter().any(|f| f == field) {
                        found = true;
                        break;
                    }
                }
            }
            if !found {
                return Err("E007".to_string());
            }
            Ok(())
        }
        Expr::Set(instance, field, index, value) => {
            verify_expr(instance, context)?;
            verify_expr(index, context)?;
            verify_expr(value, context)?;
            let inferred = infer_shape(instance, &context.stack_shapes);
            let mut found = false;
            if let Some(ref shape_name) = inferred {
                if let Some(fields) = context.shapes.get(shape_name) {
                    if fields.iter().any(|f| f == field) {
                        found = true;
                    }
                }
            } else {
                for fields in context.shapes.values() {
                    if fields.iter().any(|f| f == field) {
                        found = true;
                        break;
                    }
                }
            }
            if !found {
                return Err("E007".to_string());
            }
            Ok(())
        }
        Expr::Pack(e) => {
            verify_expr(e, context)?;
            let inferred = infer_shape(e, &context.stack_shapes);
            if let Some(ref shape_name) = inferred {
                if context.shapes.contains_key(shape_name) {
                    Ok(())
                } else {
                    Err("E006".to_string())
                }
            } else {
                Err("E006".to_string())
            }
        }
        Expr::Unpack(e, shape_name) => {
            verify_expr(e, context)?;
            if !context.shapes.contains_key(shape_name) {
                return Err("E006".to_string());
            }
            Ok(())
        }
        Expr::Map(e, field, func_expr) => {
            verify_expr(e, context)?;
            let inferred = infer_shape(e, &context.stack_shapes);
            let fields = if let Some(ref shape_name) = inferred {
                if let Some(fields) = context.shapes.get(shape_name) {
                    fields
                } else {
                    return Err("E006".to_string());
                }
            } else {
                return Err("E006".to_string());
            };
            if !fields.iter().any(|f| f == field) {
                return Err("E007".to_string());
            }
            if let Expr::Identifier(name) = &**func_expr {
                if !context.functions.contains_key(name) {
                    return Err("E010".to_string());
                }
            }
            Ok(())
        }
        Expr::Filter(e, func_expr) => {
            verify_expr(e, context)?;
            let inferred = infer_shape(e, &context.stack_shapes);
            if let Some(ref shape_name) = inferred {
                if !context.shapes.contains_key(shape_name) {
                    return Err("E006".to_string());
                }
            } else {
                return Err("E006".to_string());
            };
            if let Expr::Identifier(name) = &**func_expr {
                if !context.functions.contains_key(name) {
                    return Err("E010".to_string());
                }
            }
            Ok(())
        }
        Expr::Apply(func_expr, args) => {
            if let Expr::Identifier(name) = &**func_expr {
                if !context.functions.contains_key(name) {
                    return Err("E010".to_string());
                }
            } else {
                return Err("E012".to_string());
            }
            for arg in args {
                verify_expr(arg, context)?;
            }
            Ok(())
        }
        Expr::Trap(try_expr, fallback_expr) => {
            let mut captures_states = Vec::new();
            let mut captures_shapes = Vec::new();
            for (i, state) in context.stack.iter().enumerate() {
                if *state == VariableState::Available || *state == VariableState::Borrowed {
                    captures_states.push(VariableState::Borrowed);
                    captures_shapes.push(context.stack_shapes[i].clone());
                }
            }

            let mut try_context = VerificationContext {
                shapes: context.shapes.clone(),
                functions: context.functions.clone(),
                stack: captures_states.clone(),
                stack_shapes: captures_shapes.clone(),
                expand_map: context.expand_map.clone(),
            };
            verify_expr(try_expr, &mut try_context)?;

            let mut fallback_context = VerificationContext {
                shapes: context.shapes.clone(),
                functions: context.functions.clone(),
                stack: captures_states,
                stack_shapes: captures_shapes,
                expand_map: context.expand_map.clone(),
            };
            verify_expr(fallback_expr, &mut fallback_context)?;
            Ok(())
        }
        Expr::BinaryOp(op, left, right) => {
            match op {
                Token::Add | Token::Sub | Token::Mul | Token::Div |
                Token::Eq | Token::Lt | Token::Gt |
                Token::BitAnd | Token::BitOr | Token::BitXor => {
                    verify_expr(left, context)?;
                    verify_expr(right, context)
                }
                _ => Err("E008".to_string()),
            }
        }
        Expr::Cat(l, r) | Expr::Loc(l, r) | Expr::Reg(l, r) | Expr::Write(l, r) => {
            verify_expr(l, context)?;
            verify_expr(r, context)
        }
        Expr::Sub(s, b, l) | Expr::Split(s, b, l) => {
            verify_expr(s, context)?;
            verify_expr(b, context)?;
            verify_expr(l, context)
        }
        Expr::TimeGet(t, i) => {
            verify_expr(t, context)?;
            verify_expr(i, context)
        }
        Expr::TimeSet(y, m, d, h, mn, s) => {
            verify_expr(y, context)?;
            verify_expr(m, context)?;
            verify_expr(d, context)?;
            verify_expr(h, context)?;
            verify_expr(mn, context)?;
            verify_expr(s, context)
        }
        Expr::MoneyOp(_, l, r) | Expr::TimeOp(_, l, r) => {
            verify_expr(l, context)?;
            verify_expr(r, context)
        }
        Expr::Len(e) | Expr::Str(e) | Expr::Read(e) | Expr::Env(e) |
        Expr::MoneyStr(e) | Expr::Panic(e) => verify_expr(e, context),
        Expr::Define(_, _, body, _) => verify_expr(body, context),
        Expr::Shape(_, _, _) | Expr::Import(..) => Ok(()),
    }
}
