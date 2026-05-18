use llmlang::lexer::{Lexer};
use llmlang::parser::{Parser, Expr};
use llmlang::codegen::{CodeGen};
use inkwell::context::Context;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::collections::HashMap;

#[test]
fn test_positive_math() {
    let context = Context::create();
    let input = "+ 1 2";
    let ast = Parser::new(Lexer::new(input)).parse_expr();
    let codegen = CodeGen::new(&context, "test");
    codegen.gen_function("main", vec![], &ast);
    
    let ir = codegen.module.print_to_string().to_string();
    assert!(ir.contains("add i64 1, 2") || ir.contains("ret i64 3"));
}

#[test]
fn test_positive_debruijn() {
    let context = Context::create();
    let input = ": add_one x + ^0 1";
    let ast = Parser::new(Lexer::new(input)).parse_expr();
    let codegen = CodeGen::new(&context, "test");
    if let Expr::Define(name, params, body, _) = ast {
        codegen.gen_function(&name, params, &body);
    }
    
    let ir = codegen.module.print_to_string().to_string();
    assert!(ir.contains("define i64 @add_one(i64 %0)"));
    assert!(ir.contains("add i64 %0, 1"));
}

#[test]
fn test_positive_soa_shape() {
    let context = Context::create();
    let codegen = CodeGen::new(&context, "test");
    codegen.gen_shape("Point", &["x".to_string(), "y".to_string()]);
    
    let body = Expr::Get(
        Box::new(Expr::New("Point".to_string(), Box::new(Expr::Integer(10)))),
        "x".to_string(),
        Box::new(Expr::Integer(0))
    );
    codegen.gen_function("get_x", vec![], &body);
    
    let ir = codegen.module.print_to_string().to_string();
    assert!(ir.contains("alloca i64, i64 10"));
    assert!(ir.contains("getelementptr i64"));
    assert!(ir.contains("load i64"));
}

#[test]
fn test_positive_move_borrow() {
    let context = Context::create();
    let input = ": test x + & ^0 > ^0"; 
    let ast = Parser::new(Lexer::new(input)).parse_expr();
    let codegen = CodeGen::new(&context, "test");
    if let Expr::Define(name, params, body, _) = ast {
        codegen.gen_function(&name, params, &body);
    }
    assert!(codegen.warnings.borrow().is_empty());
}

#[test]
fn test_negative_double_move() {
    let context = Context::create();
    let input = ": test x + > ^0 > ^0";
    let ast = Parser::new(Lexer::new(input)).parse_expr();
    let codegen = CodeGen::new(&context, "test");
    if let Expr::Define(name, params, body, _) = ast {
        let result = catch_unwind(AssertUnwindSafe(|| {
            codegen.gen_function(&name, params, &body);
        }));
        assert!(result.is_err());
        let err = result.err().unwrap();
        if let Some(msg) = err.downcast_ref::<&str>() {
            assert_eq!(*msg, "E005");
        } else if let Some(msg) = err.downcast_ref::<String>() {
            assert_eq!(msg, "E005");
        }
    }
}

#[test]
fn test_negative_leak() {
    let context = Context::create();
    let input = ": leak x 42"; 
    let ast = Parser::new(Lexer::new(input)).parse_expr();
    let codegen = CodeGen::new(&context, "test");
    if let Expr::Define(name, params, body, _) = ast {
        codegen.gen_function(&name, params, &body);
    }
    assert!(codegen.warnings.borrow().contains(&"W001".to_string()));
}

#[test]
fn test_negative_out_of_bounds() {
    let context = Context::create();
    let input = ": test x ^1"; 
    let ast = Parser::new(Lexer::new(input)).parse_expr();
    let codegen = CodeGen::new(&context, "test");
    if let Expr::Define(name, params, body, _) = ast {
        let result = catch_unwind(AssertUnwindSafe(|| {
            codegen.gen_function(&name, params, &body);
        }));
        assert!(result.is_err());
        let err = result.err().unwrap();
        if let Some(msg) = err.downcast_ref::<&str>() {
            assert_eq!(*msg, "E003");
        } else if let Some(msg) = err.downcast_ref::<String>() {
            assert_eq!(msg, "E003");
        }
    }
}

#[test]
fn test_negative_unknown_shape() {
    let context = Context::create();
    let body = Expr::New("Unknown".to_string(), Box::new(Expr::Integer(1)));
    let codegen = CodeGen::new(&context, "test");
    let result = catch_unwind(AssertUnwindSafe(|| {
        codegen.gen_function("test", vec![], &body);
    }));
    assert!(result.is_err());
    let err = result.err().unwrap();
    if let Some(msg) = err.downcast_ref::<&str>() {
        assert_eq!(*msg, "E006");
    } else if let Some(msg) = err.downcast_ref::<String>() {
        assert_eq!(msg, "E006");
    }
}

#[test]
fn test_positive_branching() {
    let context = Context::create();
    let input = ": test x ? ^0 1 0"; 
    let ast = Parser::new(Lexer::new(input)).parse_expr();
    let codegen = CodeGen::new(&context, "test");
    if let Expr::Define(name, params, body, _) = ast {
        codegen.gen_function(&name, params, &body);
    }
    
    let ir = codegen.module.print_to_string().to_string();
    assert!(ir.contains("then:"));
    assert!(ir.contains("else:"));
}

#[test]
fn test_negative_branch_mismatch() {
    let context = Context::create();
    let input = ": test x ? ^0 > ^0 0"; 
    let ast = Parser::new(Lexer::new(input)).parse_expr();
    let codegen = CodeGen::new(&context, "test");
    if let Expr::Define(name, params, body, _) = ast {
        let result = catch_unwind(AssertUnwindSafe(|| {
            codegen.gen_function(&name, params, &body);
        }));
        assert!(result.is_err());
        let err = result.err().unwrap();
        if let Some(msg) = err.downcast_ref::<&str>() {
            assert_eq!(*msg, "E009");
        } else if let Some(msg) = err.downcast_ref::<String>() {
            assert_eq!(msg, "E009");
        }
    }
}

#[test]
fn test_positive_recursion() {
    let context = Context::create();
    let input = ": fact n ? ^0 * & ^0 @ fact - > ^0 1 > ^0";
    let ast = Parser::new(Lexer::new(input)).parse_expr();
    let codegen = CodeGen::new(&context, "test");
    if let Expr::Define(name, params, body, _) = ast {
        codegen.gen_function(&name, params, &body);
    }
    
    let ir = codegen.module.print_to_string().to_string();
    assert!(ir.contains("call i64 @fact"));
}

#[test]
fn test_positive_expansion() {
    let context = Context::create();
    let input = ": poly !obj get !obj x 0";
    let ast = Parser::new(Lexer::new(input)).parse_expr();
    let codegen = CodeGen::new(&context, "test");
    codegen.gen_shape("Point", &["x".to_string()]);
    
    if let Expr::Define(name, params, body, _) = ast {
        codegen.gen_function(&name, params, &body);
    }

    let call_input = "@ poly new Point 1";
    let call_ast = Parser::new(Lexer::new(call_input)).parse_expr();
    codegen.gen_function("wrapper", vec![], &call_ast);

    let ir = codegen.module.print_to_string().to_string();
    assert!(ir.contains("alloca i64"));
    assert!(ir.contains("getelementptr"));
}

#[test]
fn test_positive_export_sig() {
    let context = Context::create();
    let input = "export # Point x y\nexport : add_x !obj get !obj x 0";
    let mut parser = Parser::new(Lexer::new(input));
    let codegen = CodeGen::new(&context, "test");
    
    let exprs = parser.parse_module();
    for expr in exprs {
        match expr {
            Expr::Shape(n, f, _) => codegen.gen_shape(&n, &f),
            Expr::Define(n, p, b, _) => { codegen.gen_function(&n, p, &b); },
            _ => {}
        }
    }

    let sig = codegen.emit_signature_file();
    assert!(sig.contains("# Point x y"));
    assert!(sig.contains(": add_x ..."));
}
