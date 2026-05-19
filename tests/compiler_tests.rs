use llmlang::lexer::{Lexer};
use llmlang::parser::{Parser, Expr};
use llmlang::codegen::{CodeGen};
use inkwell::context::Context;
use std::panic::{catch_unwind, AssertUnwindSafe};

#[test]
fn test_positive_math() {
    let context = Context::create();
    let input = "+ 1 2";
    let ast = Parser::new(Lexer::new(input), "test.llm".to_string()).parse_expr();
    let codegen = CodeGen::new(&context, "test");
    codegen.gen_function("main", vec![], &ast);
    let ir = codegen.module.print_to_string().to_string();
    assert!(ir.contains("add i64 1, 2") || ir.contains("ret i64 3"));
}

#[test]
fn test_positive_div() {
    let context = Context::create();
    let input = "/ 10 2";
    let ast = Parser::new(Lexer::new(input), "test.llm".to_string()).parse_expr();
    let codegen = CodeGen::new(&context, "test");
    codegen.gen_function("main", vec![], &ast);
    let ir = codegen.module.print_to_string().to_string();
    assert!(ir.contains("sdiv i64 10, 2") || ir.contains("ret i64 5"));
}

#[test]
fn test_positive_comparisons() {
    let context = Context::create();
    let input = ": main x y < ^1 ^0";
    let mut parser = Parser::new(Lexer::new(input), "test.llm".to_string());
    let codegen = CodeGen::new(&context, "test");
    if let Expr::Define(name, params, body, _) = parser.parse_module()[0].clone() {
        codegen.gen_function(&name, params, &body);
    }
    let ir = codegen.module.print_to_string().to_string();
    assert!(ir.contains("icmp slt i64 %0, %1"));
    assert!(ir.contains("zext i1 %lttmp to i64"));
}

#[test]
fn test_positive_bitwise() {
    let context = Context::create();
    let input = ": main x y | & ^1 ^0 ^ ^1 ^0";
    let mut parser = Parser::new(Lexer::new(input), "test.llm".to_string());
    let codegen = CodeGen::new(&context, "test");
    if let Expr::Define(name, params, body, _) = parser.parse_module()[0].clone() {
        codegen.gen_function(&name, params, &body);
    }
    let ir = codegen.module.print_to_string().to_string();
    assert!(ir.contains("and i64 %0, %1"));
    assert!(ir.contains("xor i64 %0, %1"));
    assert!(ir.contains("or i64 %andtmp, %xortmp"));
}

#[test]
fn test_positive_debruijn() {
    let context = Context::create();
    let input = ": add_one x + ^0 1";
    let ast = Parser::new(Lexer::new(input), "test.llm".to_string()).parse_expr();
    let codegen = CodeGen::new(&context, "test");
    if let Expr::Define(name, params, body, _) = ast {
        codegen.gen_function(&name, params, &body);
    }
    let ir = codegen.module.print_to_string().to_string();
    assert!(ir.contains("define i64 @add_one(i64 %0)"));
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
}

#[test]
fn test_positive_move_borrow() {
    let context = Context::create();
    let input = ": test x + ⚓ ^0 ⮞ ^0"; 
    let ast = Parser::new(Lexer::new(input), "test.llm".to_string()).parse_expr();
    let codegen = CodeGen::new(&context, "test");
    if let Expr::Define(name, params, body, _) = ast {
        codegen.gen_function(&name, params, &body);
    }
    assert!(codegen.warnings.borrow().is_empty());
}

#[test]
fn test_negative_double_move() {
    let context = Context::create();
    let input = ": test x + ⮞ ^0 ⮞ ^0";
    let ast = Parser::new(Lexer::new(input), "test.llm".to_string()).parse_expr();
    let codegen = CodeGen::new(&context, "test");
    if let Expr::Define(name, params, body, _) = ast {
        let result = catch_unwind(AssertUnwindSafe(|| {
            codegen.gen_function(&name, params, &body);
        }));
        assert!(result.is_err());
        assert_eq!(result.err().unwrap().downcast_ref::<&str>().unwrap(), &"E005");
    }
}

#[test]
fn test_positive_let() {
    let context = Context::create();
    let input = ": test x L y + ^0 1 ⮞ ^0"; // n is at ^1 now
    let ast = Parser::new(Lexer::new(input), "test.llm".to_string()).parse_expr();
    let codegen = CodeGen::new(&context, "test");
    if let Expr::Define(name, params, body, _) = ast {
        codegen.gen_function(&name, params, &body);
    }
    let ir = codegen.module.print_to_string().to_string();
    assert!(ir.contains("add i64 %0, 1"));
}

#[test]
fn test_positive_recursion() {
    let context = Context::create();
    let input = ": fact n ? ^0 * ⚓ ^0 @ fact - ⮞ ^0 1 ⮞ ^0";
    let ast = Parser::new(Lexer::new(input), "test.llm".to_string()).parse_expr();
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
    let input = ": poly !obj G !obj x 0";
    let ast = Parser::new(Lexer::new(input), "test.llm".to_string()).parse_expr();
    let codegen = CodeGen::new(&context, "test");
    codegen.gen_shape("Point", &["x".to_string()]);
    if let Expr::Define(name, params, body, _) = ast {
        codegen.gen_function(&name, params, &body);
    }
    let call_input = "@ poly N Point 1";
    let call_ast = Parser::new(Lexer::new(call_input), "test.llm".to_string()).parse_expr();
    codegen.gen_function("wrapper", vec![], &call_ast);
    let ir = codegen.module.print_to_string().to_string();
    assert!(ir.contains("alloca i64"));
}

#[test]
fn test_positive_export_sig() {
    let context = Context::create();
    let input = "X # Point x y\nX : add_x !obj G !obj x 0";
    let mut parser = Parser::new(Lexer::new(input), "test.llm".to_string());
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

#[test]
fn test_positive_fingerprint() {
    let input1 = ": f1 x + ⮞ ^0 1";
    let input2 = ": f2 y + ⮞ ^0 1";
    let input3 = ": f3 x * ⮞ ^0 2";

    let ast1 = Parser::new(Lexer::new(input1), "test1.llm".to_string()).parse_expr();
    let ast2 = Parser::new(Lexer::new(input2), "test2.llm".to_string()).parse_expr();
    let ast3 = Parser::new(Lexer::new(input3), "test3.llm".to_string()).parse_expr();

    let fp1 = ast1.structural_fingerprint();
    let fp2 = ast2.structural_fingerprint();
    let fp3 = ast3.structural_fingerprint();

    // f1 and f2 should have identical fingerprints despite name changes
    assert_eq!(fp1, fp2);
    // f3 should be different
    assert_ne!(fp1, fp3);
}

#[test]
fn test_positive_import() {
    let context = Context::create();
    let input = "I math sin\n: test x @ sin ^0";
    let mut parser = Parser::new(Lexer::new(input), "test.llm".to_string());
    let codegen = CodeGen::new(&context, "test");
    
    let exprs = parser.parse_module();
    for expr in exprs {
        match expr {
            Expr::Import(m, s) => codegen.gen_import(&m, &s),
            Expr::Define(n, p, b, _) => { codegen.gen_function(&n, p, &b); },
            _ => {}
        }
    }
    
    let ir = codegen.module.print_to_string().to_string();
    assert!(ir.contains("declare i64 @sin(i64)"));
    assert!(ir.contains("call i64 @sin(i64 %0)"));
}

#[test]
fn test_positive_multi_arity_parsing() {
    let input = "@2 add2 1 2";
    let ast = Parser::new(Lexer::new(input), "test.llm".to_string()).parse_expr();
    if let Expr::Apply(func, args) = ast {
        if let Expr::Identifier(name) = *func {
            assert_eq!(name, "add2");
        } else {
            panic!("Expected identifier");
        }
        assert_eq!(args.len(), 2);
    } else {
        panic!("Expected Apply");
    }
}

#[test]
fn test_positive_multi_arity_codegen() {
    let context = Context::create();
    let input = ": add2 x y + ^1 ^0\n: main @2 add2 10 20";
    let mut parser = Parser::new(Lexer::new(input), "test.llm".to_string());
    let codegen = CodeGen::new(&context, "test");
    
    let exprs = parser.parse_module();
    for expr in exprs {
        match expr {
            Expr::Define(name, params, body, _) => {
                codegen.gen_function(&name, params, &body);
            }
            _ => {}
        }
    }
    
    let ir = codegen.module.print_to_string().to_string();
    assert!(ir.contains("define i64 @add2(i64 %0, i64 %1)"));
    assert!(ir.contains("call i64 @add2(i64 10, i64 20)"));
}

#[test]
fn test_positive_nested_multi_arity() {
    let context = Context::create();
    // f(x, y, z) = x + (y * z)
    // @3 f 1 2 3
    let input = ": f x y z + ^2 * ^1 ^0\n: main @3 f 1 2 3";
    let mut parser = Parser::new(Lexer::new(input), "test.llm".to_string());
    let codegen = CodeGen::new(&context, "test");
    
    let exprs = parser.parse_module();
    for expr in exprs {
        match expr {
            Expr::Define(name, params, body, _) => {
                codegen.gen_function(&name, params, &body);
            }
            _ => {}
        }
    }
    
    let ir = codegen.module.print_to_string().to_string();
    assert!(ir.contains("define i64 @f(i64 %0, i64 %1, i64 %2)"));
    assert!(ir.contains("call i64 @f(i64 1, i64 2, i64 3)"));
}

#[test]
fn test_positive_fingerprint_arity() {
    let input1 = ": f1 x @ g ^0";
    let input2 = ": f2 x y @2 g ^1 ^0";

    let ast1 = Parser::new(Lexer::new(input1), "test1.llm".to_string()).parse_expr();
    let ast2 = Parser::new(Lexer::new(input2), "test2.llm".to_string()).parse_expr();

    let fp1 = ast1.structural_fingerprint();
    let fp2 = ast2.structural_fingerprint();

    // f1 and f2 should have different fingerprints due to arity
    assert_ne!(fp1, fp2);
    assert!(fp1.contains("@1"));
    assert!(fp2.contains("@2"));
}
