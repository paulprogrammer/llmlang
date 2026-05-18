use llmlang::lexer::{Lexer};
use llmlang::parser::{Parser, Expr};
use llmlang::codegen::{CodeGen};
use inkwell::context::Context;
use std::panic::{catch_unwind, AssertUnwindSafe};

#[test]
fn test_positive_math() {
    let context = Context::create();
    let input = "+ 1 2";
    let ast = Parser::new(Lexer::new(input)).parse_expr();
    let codegen = CodeGen::new(&context, "test");
    codegen.gen_function("main", 0, &ast);
    
    let ir = codegen.module.print_to_string().to_string();
    assert!(ir.contains("add i64 1, 2") || ir.contains("ret i64 3"));
}

#[test]
fn test_positive_debruijn() {
    let context = Context::create();
    let input = ": add_one x + ^0 1";
    let ast = Parser::new(Lexer::new(input)).parse_expr();
    let codegen = CodeGen::new(&context, "test");
    if let Expr::Define(name, args, body) = ast {
        codegen.gen_function(&name, args.len(), &body);
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
    codegen.gen_function("get_x", 0, &body);
    
    let ir = codegen.module.print_to_string().to_string();
    assert!(ir.contains("alloca i64, i64 10"));
    assert!(ir.contains("getelementptr i64"));
    assert!(ir.contains("load i64"));
}

#[test]
fn test_positive_move_borrow() {
    let context = Context::create();
    let input = ": test x + & ^0 > ^0"; // Borrow then move
    let ast = Parser::new(Lexer::new(input)).parse_expr();
    let codegen = CodeGen::new(&context, "test");
    if let Expr::Define(name, args, body) = ast {
        codegen.gen_function(&name, args.len(), &body);
    }
    assert!(codegen.warnings.borrow().is_empty());
}

#[test]
fn test_negative_double_move() {
    let context = Context::create();
    let input = ": test x + > ^0 > ^0";
    let ast = Parser::new(Lexer::new(input)).parse_expr();
    let codegen = CodeGen::new(&context, "test");
    if let Expr::Define(name, args, body) = ast {
        let result = catch_unwind(AssertUnwindSafe(|| {
            codegen.gen_function(&name, args.len(), &body);
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
    if let Expr::Define(name, args, body) = ast {
        codegen.gen_function(&name, args.len(), &body);
    }
    assert!(codegen.warnings.borrow().contains(&"W001".to_string()));
}

#[test]
fn test_negative_out_of_bounds() {
    let context = Context::create();
    let input = ": test x ^1"; // Only 1 arg, index 1 is out of bounds
    let ast = Parser::new(Lexer::new(input)).parse_expr();
    let codegen = CodeGen::new(&context, "test");
    if let Expr::Define(name, args, body) = ast {
        let result = catch_unwind(AssertUnwindSafe(|| {
            codegen.gen_function(&name, args.len(), &body);
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
        codegen.gen_function("test", 0, &body);
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
    let input = ": test x ? ^0 1 0"; // if x != 0 then 1 else 0
    let ast = Parser::new(Lexer::new(input)).parse_expr();
    let codegen = CodeGen::new(&context, "test");
    if let Expr::Define(name, args, body) = ast {
        codegen.gen_function(&name, args.len(), &body);
    }
    
    let ir = codegen.module.print_to_string().to_string();
    assert!(ir.contains("then:"));
    assert!(ir.contains("else:"));
    assert!(ir.contains("phi i64 [ 1, %then ], [ 0, %else ]"));
}

#[test]
fn test_positive_recursion() {
    let context = Context::create();
    // Equivalent to: 
    // def fact(n):
    //   if n != 0: return n * fact(n - 1)
    //   else: return 1
    
    // In llmlang:
    // : fact n ? ^0 * & ^0 @ fact - > ^0 1 > ^0
    
    let input = ": fact n ? ^0 * & ^0 @ fact - > ^0 1 > ^0";
    let ast = Parser::new(Lexer::new(input)).parse_expr();
    let codegen = CodeGen::new(&context, "test");
    if let Expr::Define(name, args, body) = ast {
        codegen.gen_function(&name, args.len(), &body);
    }
    
    let ir = codegen.module.print_to_string().to_string();
    assert!(ir.contains("call i64 @fact"));
    assert!(ir.contains("phi i64"));
}
