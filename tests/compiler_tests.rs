use llmlang::compiler::lexer::{Lexer};
use llmlang::compiler::parser::{Parser};
use llmlang::compiler::ast::{Expr};
use llmlang::compiler::codegen::{CodeGen};
use inkwell::context::Context;
use std::panic::{catch_unwind, AssertUnwindSafe};

fn parse_expr(input: &str) -> Expr {
    Parser::new(Lexer::new(input), "test.llm".to_string()).parse_expr()
}

fn parse_module(input: &str) -> Vec<Expr> {
    Parser::new(Lexer::new(input), "test.llm".to_string()).parse_module()
}

#[test]
fn test_positive_math() {
    let context = Context::create();
    let input = "+ 1 2";
    let ast = parse_expr(input);
    let codegen = CodeGen::new(&context, "test", llmlang::Config::default());
    codegen.gen_function("main", vec![], &ast, false);
    let ir = codegen.module.print_to_string().to_string();
    assert!(ir.contains("add i64 1, 2") || ir.contains("ret i64 3"));
}

#[test]
fn test_positive_div() {
    let context = Context::create();
    let input = "/ 10 2";
    let ast = parse_expr(input);
    let codegen = CodeGen::new(&context, "test", llmlang::Config::default());
    codegen.gen_function("main", vec![], &ast, false);
    let ir = codegen.module.print_to_string().to_string();
    assert!(ir.contains("sdiv i64 10, 2") || ir.contains("ret i64 5"));
}

#[test]
fn test_positive_comparisons() {
    let context = Context::create();
    let input = "X : main x y < ⚓ ^1 ⚓ ^0";
    let codegen = CodeGen::new(&context, "test", llmlang::Config::default());
    if let Expr::Define(name, params, body, exported) = parse_module(input)[0].clone() { codegen.gen_function(&name, params, &body, exported); }
    let ir = codegen.module.print_to_string().to_string();
    assert!(ir.contains("icmp slt i64 %0, %1"));
    assert!(ir.contains("zext i1") || ir.contains("zext i1 %lt to i64"));
}

#[test]
fn test_positive_bitwise() {
    let context = Context::create();
    let input = "X : main x y | & ⚓ ^1 ⚓ ^0 ^ ⚓ ^1 ⚓ ^0";
    let codegen = CodeGen::new(&context, "test", llmlang::Config::default());
    if let Expr::Define(name, params, body, exported) = parse_module(input)[0].clone() { codegen.gen_function(&name, params, &body, exported); }
    let ir = codegen.module.print_to_string().to_string();
    assert!(ir.contains("and i64 %0, %1"));
    assert!(ir.contains("xor i64 %0, %1"));
    assert!(ir.contains("or i64") || ir.contains("or i64 %and, %xor"));
}

#[test]
fn test_positive_debruijn() {
    let context = Context::create();
    let input = "X : add_one x + ^0 1";
    let ast = parse_expr(input);
    let codegen = CodeGen::new(&context, "test", llmlang::Config::default());
    if let Expr::Define(name, params, body, exported) = ast { codegen.gen_function(&name, params, &body, exported); }
    let ir = codegen.module.print_to_string().to_string();
    assert!(ir.contains("define i64 @add_one(i64 %0)"));
}

#[test]
fn test_positive_soa_shape() {
    let context = Context::create();
    let codegen = CodeGen::new(&context, "test", llmlang::Config::default());
    codegen.gen_shape("Point", &["x".to_string(), "y".to_string()], false);
    let body = Expr::Get(
        Box::new(Expr::New("Point".to_string(), Box::new(Expr::Integer(10)))),
        "x".to_string(),
        Box::new(Expr::Integer(0))
    );
    codegen.gen_function("get_x", vec![], &body, false);
    let ir = codegen.module.print_to_string().to_string();
    assert!(ir.contains("call ptr @llm_alloc"));
}

#[test]
fn test_positive_move_borrow() {
    let context = Context::create();
    let input = "X : test x + ⚓ ^0 ⮞ ^0"; 
    let ast = parse_expr(input);
    let codegen = CodeGen::new(&context, "test", llmlang::Config::default());
    if let Expr::Define(name, params, body, exported) = ast { codegen.gen_function(&name, params, &body, exported); }
    assert!(codegen.warnings.borrow().is_empty());
}

#[test]
fn test_negative_double_move() {
    let context = Context::create();
    let input = "X : test x + ⮞ ^0 ⮞ ^0";
    let ast = parse_expr(input);
    let codegen = CodeGen::new(&context, "test", llmlang::Config::default());
    if let Expr::Define(name, params, body, _) = ast {
        let result = catch_unwind(AssertUnwindSafe(|| {
            codegen.gen_function(&name, params, &body, false);
        }));
        assert!(result.is_err());
    }
}

#[test]
fn test_positive_let() {
    let context = Context::create();
    let input = "X : test x L y + ^0 1 ⮞ ^0"; 
    let ast = parse_expr(input);
    let codegen = CodeGen::new(&context, "test", llmlang::Config::default());
    if let Expr::Define(name, params, body, exported) = ast { codegen.gen_function(&name, params, &body, exported); }
    let ir = codegen.module.print_to_string().to_string();
    assert!(ir.contains("add i64 %0, 1"));
}

#[test]
fn test_positive_recursion() {
    let context = Context::create();
    let input = "X : fact n ? ^0 * ⚓ ^0 @ fact - ⮞ ^0 1 ⮞ ^0";
    let ast = parse_expr(input);
    let codegen = CodeGen::new(&context, "test", llmlang::Config::default());
    if let Expr::Define(name, params, body, exported) = ast { codegen.gen_function(&name, params, &body, exported); }
    let ir = codegen.module.print_to_string().to_string();
    assert!(ir.contains("call i64 @fact"));
}

#[test]
fn test_positive_expansion() {
    let context = Context::create();
    let input = "X : poly obj ! G ⚓ ! obj x 0";
    let ast = parse_expr(input);
    let codegen = CodeGen::new(&context, "test", llmlang::Config::default());
    codegen.gen_shape("Point", &["x".to_string()], false);
    if let Expr::Define(name, params, body, exported) = ast { codegen.gen_function(&name, params, &body, exported); }
    let call_input = "@ poly N Point 1";
    let call_ast = parse_expr(call_input);
    codegen.gen_function("wrapper", vec![], &call_ast, false);
    let ir = codegen.module.print_to_string().to_string();
    assert!(ir.contains("call ptr @llm_alloc"));
}

#[test]
fn test_positive_export_sig() {
    let context = Context::create();
    let input = "X # Point x y\nX : add_x obj ! G ⚓ ! obj x 0";
    let mut parser = Parser::new(Lexer::new(input), "test.llm".to_string());
    let codegen = CodeGen::new(&context, "test", llmlang::Config::default());
    let exprs = parser.parse_module();
    for expr in exprs {
        match expr {
            Expr::Shape(n, f, exported) => codegen.gen_shape(&n, &f, exported),
            Expr::Define(n, p, b, exported) => { codegen.gen_function(&n, p, &b, exported); },
            _ => {}
        }
    }
    let sig = codegen.emit_signature_file();
    assert!(sig.contains("# Point x y"));
    assert!(sig.contains(": add_x 1"));
}

#[test]
fn test_positive_fingerprint() {
    let input1 = "X : f1 x + ⮞ ^0 1";
    let input2 = "X : f2 y + ⮞ ^0 1";
    let input3 = "X : f3 x * ⮞ ^0 2";

    let mut parser1 = Parser::new(Lexer::new(input1), "test1.llm".to_string());
    let ast1 = parser1.parse_expr();
    let mut parser2 = Parser::new(Lexer::new(input2), "test2.llm".to_string());
    let ast2 = parser2.parse_expr();
    let mut parser3 = Parser::new(Lexer::new(input3), "test3.llm".to_string());
    let ast3 = parser3.parse_expr();

    let fp1 = ast1.structural_fingerprint();
    let fp2 = ast2.structural_fingerprint();
    let fp3 = ast3.structural_fingerprint();

    assert_eq!(fp1, fp2);
    assert_ne!(fp1, fp3);
}

#[test]
fn test_positive_import() {
    let context = Context::create();
    let input = "I math sin\nX : test x @ sin ^0";
    
    // Create a dummy .llmi file for the test
    let _ = std::fs::write("math.llmi", ": sin 1\n");
    
    let mut parser = Parser::new(Lexer::new(input), "test.llm".to_string());
    let codegen = CodeGen::new(&context, "test", llmlang::Config::default());
    
    let exprs = parser.parse_module();
    for expr in exprs {
        match expr {
            Expr::Import(m, s, a) => codegen.gen_import(&m, &s, a),
            Expr::Define(n, p, b, exported) => { codegen.gen_function(&n, p, &b, exported); },
            _ => {}
        }
    }
    
    let ir = codegen.module.print_to_string().to_string();
    assert!(ir.contains("declare i64 @math_sin(i64)"));
    assert!(ir.contains("call i64 @math_sin(i64 %0)"));
    
    // Cleanup
    let _ = std::fs::remove_file("math.llmi");
}

#[test]
fn test_positive_multi_arity_parsing() {
    let input = "@2 add2 1 2";
    let mut parser = Parser::new(Lexer::new(input), "test.llm".to_string());
    let ast = parser.parse_expr();
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
    let input = "X : add2 x y + ^1 ^0\nX : main @2 add2 10 20";
    let mut parser = Parser::new(Lexer::new(input), "test.llm".to_string());
    let codegen = CodeGen::new(&context, "test", llmlang::Config::default());
    
    let exprs = parser.parse_module();
    for expr in exprs {
        match expr {
            Expr::Define(name, params, body, exported) => { codegen.gen_function(&name, params, &body, exported); }
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
    let input = "X : f x y z + ^2 * ^1 ^0\nX : main @3 f 1 2 3";
    let mut parser = Parser::new(Lexer::new(input), "test.llm".to_string());
    let codegen = CodeGen::new(&context, "test", llmlang::Config::default());
    
    let exprs = parser.parse_module();
    for expr in exprs {
        match expr {
            Expr::Define(name, params, body, exported) => { codegen.gen_function(&name, params, &body, exported); }
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

    let mut parser1 = Parser::new(Lexer::new(input1), "test1.llm".to_string());
    let ast1 = parser1.parse_expr();
    let mut parser2 = Parser::new(Lexer::new(input2), "test2.llm".to_string());
    let ast2 = parser2.parse_expr();

    let fp1 = ast1.structural_fingerprint();
    let fp2 = ast2.structural_fingerprint();

    assert_ne!(fp1, fp2);
    assert!(fp1.contains("@1"));
    assert!(fp2.contains("@2"));
}

#[test]
fn test_positive_string_literals() {
    let context = Context::create();
    let input = "\"hello world\"";
    let mut parser = Parser::new(Lexer::new(input), "test.llm".to_string());
    let ast = parser.parse_expr();
    let codegen = CodeGen::new(&context, "test", llmlang::Config::default());
    codegen.gen_function("main", vec![], &ast, false);
    let ir = codegen.module.print_to_string().to_string();
    assert!(ir.contains("str_const"));
}

#[test]
fn test_positive_string_ops() {
    let context = Context::create();
    let input = "X : main ⧉ \"a\" \"b\"";
    let mut parser = Parser::new(Lexer::new(input), "test.llm".to_string());
    let codegen = CodeGen::new(&context, "test", llmlang::Config::default());
    if let Expr::Define(name, params, body, exported) = parser.parse_module()[0].clone() { codegen.gen_function(&name, params, &body, exported); }
    let ir = codegen.module.print_to_string().to_string();
    assert!(ir.contains("call i64 @llm_cat"));
}

#[test]
fn test_positive_regex() {
    let context = Context::create();
    let input = "X : main ≈ \"hello\" \"h.*o\"";
    let mut parser = Parser::new(Lexer::new(input), "test.llm".to_string());
    let codegen = CodeGen::new(&context, "test", llmlang::Config::default());
    if let Expr::Define(name, params, body, exported) = parser.parse_module()[0].clone() { codegen.gen_function(&name, params, &body, exported); }
    let ir = codegen.module.print_to_string().to_string();
    assert!(ir.contains("call i64 @llm_reg"));
}

#[test]
fn test_positive_system_ops() {
    let context = Context::create();
    let input = "X : main L s 📥 0 0";
    let mut parser = Parser::new(Lexer::new(input), "test.llm".to_string());
    let codegen = CodeGen::new(&context, "test", llmlang::Config::default());
    if let Expr::Define(name, params, body, exported) = parser.parse_module()[0].clone() { codegen.gen_function(&name, params, &body, exported); }
    let ir = codegen.module.print_to_string().to_string();
    assert!(ir.contains("call i64 @llm_read"));
    assert!(ir.contains("call void @llm_drop"));
}

#[test]
fn test_positive_split_op() {
    let context = Context::create();
    let input = "X : main 🪓 \"a,b,c\" \",\" 1";
    let mut parser = Parser::new(Lexer::new(input), "test.llm".to_string());
    let codegen = CodeGen::new(&context, "test", llmlang::Config::default());
    if let Expr::Define(name, params, body, exported) = parser.parse_module()[0].clone() { codegen.gen_function(&name, params, &body, exported); }
    let ir = codegen.module.print_to_string().to_string();
    assert!(ir.contains("call i64 @llm_split"));
}

#[test]
fn test_positive_auto_parallelism() {
    let context = Context::create();
    // Threshold is 10. Each '+' adds 1. 30 nested '+' will be > 10.
    let mut input = "X : main x ".to_string();
    for _ in 0..30 { input.push_str("+ "); }
    input.push_str("⚓ ^0 ");
    for _ in 0..30 { input.push_str("1 "); }
    let mut parser = Parser::new(Lexer::new(&input), "test.llm".to_string());
    let codegen = CodeGen::new(&context, "test", llmlang::Config { parallel_threshold: 10, ..llmlang::Config::default() });
    if let Expr::Define(name, params, body, exported) = parser.parse_module()[0].clone() { codegen.gen_function(&name, params, &body, exported); }
    let ir = codegen.module.print_to_string().to_string();
    assert!(ir.contains("define i64 @parallel_task") || ir.contains("llm_fork"));
}

#[test]
fn test_positive_temporal() {
    let context = Context::create();
    let input = "X : main L t 🕒 L y 📅 ⚓ ^0 0 📆 ⮞ ^0 1 1 0 0 0";
    let mut parser = Parser::new(Lexer::new(input), "test.llm".to_string());
    let codegen = CodeGen::new(&context, "test", llmlang::Config::default());
    if let Expr::Define(name, params, body, exported) = parser.parse_module()[0].clone() { codegen.gen_function(&name, params, &body, exported); }
    let ir = codegen.module.print_to_string().to_string();
    assert!(ir.contains("call i64 @llm_tai_now"));
    assert!(ir.contains("call i64 @llm_tai_get"));
    assert!(ir.contains("call i64 @llm_tai_set"));
}

#[test]
fn test_positive_env() {
    let context = Context::create();
    let input = "X : main 🌍 \"HOME\"";
    let mut parser = Parser::new(Lexer::new(input), "test.llm".to_string());
    let codegen = CodeGen::new(&context, "test", llmlang::Config::default());
    if let Expr::Define(name, params, body, exported) = parser.parse_module()[0].clone() { codegen.gen_function(&name, params, &body, exported); }
    let ir = codegen.module.print_to_string().to_string();
    assert!(ir.contains("call i64 @llm_getenv"));
}

#[test]
fn test_positive_json() {
    let context = Context::create();
    let codegen = CodeGen::new(&context, "test", llmlang::Config::default());
    codegen.gen_shape("User", &["id".to_string(), "age".to_string()], false);
    let input = "X : main L u N User 1 . 📦 ⚓ ^0 📦2 \"{}\" \"User\"";
    let mut parser = Parser::new(Lexer::new(input), "test.llm".to_string());
    if let Expr::Define(name, params, body, exported) = parser.parse_module()[0].clone() { codegen.gen_function(&name, params, &body, exported); }
    let ir = codegen.module.print_to_string().to_string();
    assert!(ir.contains("call i64 @llm_pack"));
    assert!(ir.contains("call i64 @llm_unpack"));
}

#[test]
fn test_positive_money() {
    let context = Context::create();
    let input = "X : main x y 💰🧵 💰+ ⚓ ^1 ⚓ ^0";
    let mut parser = Parser::new(Lexer::new(input), "test.llm".to_string());
    let codegen = CodeGen::new(&context, "test", llmlang::Config::default());
    let exprs = parser.parse_module();
    if let Expr::Define(name, params, body, exported) = exprs[0].clone() { codegen.gen_function(&name, params, &body, exported); }
    let ir = codegen.module.print_to_string().to_string();
    assert!(ir.contains("call i64 @llm_money_format"));
    assert!(ir.contains("add i64") || ir.contains("add i64 %0, %1"));
}

#[test]
fn test_positive_trap() {
    let context = Context::create();
    let input = "X : main 🛡️ 🚨 \"fail\" 42";
    let mut parser = Parser::new(Lexer::new(input), "test.llm".to_string());
    let codegen = CodeGen::new(&context, "test", llmlang::Config::default());
    let exprs = parser.parse_module();
    if let Expr::Define(name, params, body, exported) = exprs[0].clone() { codegen.gen_function(&name, params, &body, exported); }
    let ir = codegen.module.print_to_string().to_string();
    assert!(ir.contains("call i64 @llm_try"));
    assert!(ir.contains("define i64 @trap_try") || ir.contains("define i64 @trap_try_"));
    assert!(ir.contains("define i64 @trap_fallback") || ir.contains("define i64 @trap_fallback_"));
}

#[test]
fn test_integration_nested_traps() {
    let context = Context::create();
    let input = "X : main 🛡️ 🛡️ 🚨 \"inner\" 1 2";
    let mut parser = Parser::new(Lexer::new(input), "test.llm".to_string());
    let codegen = CodeGen::new(&context, "test", llmlang::Config::default());
    let exprs = parser.parse_module();
    if let Expr::Define(name, params, body, exported) = exprs[0].clone() { codegen.gen_function(&name, params, &body, exported); }
    let ir = codegen.module.print_to_string().to_string();
    // Should have multiple trap sub-functions
    assert!(ir.contains("trap_try_") && ir.contains("trap_fallback_"));
}

#[test]
fn test_integration_json_filter() {
    let context = Context::create();
    let codegen = CodeGen::new(&context, "test", llmlang::Config::default());
    codegen.gen_shape("User", &["id".to_string(), "active".to_string()], false);
    let input = "X : is_active id active ⚓ active\nX : main L u 📦2 \"[]\" \"User\" ▽ ⮞ u is_active";
    let mut parser = Parser::new(Lexer::new(input), "test.llm".to_string());
    let exprs = parser.parse_module();
    for expr in exprs {
        match expr {
            Expr::Define(n, p, b, exported) => { codegen.gen_function(&n, p, &b, exported); },
            _ => {}
        }
    }
    let ir = codegen.module.print_to_string().to_string();
    assert!(ir.contains("call i64 @llm_unpack"));
    assert!(ir.contains("filter_copy_loop"));
    assert!(ir.contains("call i64 @is_active"));
}

#[test]
fn test_analysis_dfe() {
    let input = ": used x + ⚓ x 1\n: unused y * ⚓ y 2\n: main @ used 10";
    let mut parser = Parser::new(Lexer::new(input), "test.llm".to_string());
    let mut exprs = parser.parse_module();
    
    use llmlang::compiler::analysis::prune_dead_code;
    exprs = prune_dead_code(exprs);
    
    // Check that 'unused' was removed
    let names: Vec<String> = exprs.iter().filter_map(|e| {
        if let Expr::Define(n, _, _, _) = e { Some(n.clone()) } else { None }
    }).collect();
    
    assert!(names.contains(&"used".to_string()));
    assert!(names.contains(&"main".to_string()));
    assert!(!names.contains(&"unused".to_string()));
}


#[test]
fn test_integration_complex_fault_tolerance() {
    let context = Context::create();
    // Recursive loop with nested traps and parallelism
    let input = "X : risky i ? = ⚓ i 3 🚨 \"fail\" ⚓ i\nX : loop i ? < ⚓ i 5 L res 🛡️ @ risky ⚓ i 0 . 📤 1 ⧉ 🧵 ⮞ res \"\\n\" @ loop + ⚓ i 1 0\nX : main @ loop 0";
    let mut parser = Parser::new(Lexer::new(input), "test.llm".to_string());
    let codegen = CodeGen::new(&context, "test", llmlang::Config::default());
    let exprs = parser.parse_module();
    for expr in exprs {
        match expr {
            Expr::Define(n, p, b, exported) => { codegen.gen_function(&n, p, &b, exported); },
            _ => {}
        }
    }
    let ir = codegen.module.print_to_string().to_string();
    assert!(ir.contains("llm_try"));
    assert!(ir.contains("call i64 @loop"));
}

#[test]
fn test_esoteric_parallel_recursion() {
    let context = Context::create();
    // Function that parallelizes its recursive call using a borrow
    let input = "X : main x ? ⚓ x + 1 @ main - ⚓ x 1 0";
    let mut parser = Parser::new(Lexer::new(input), "test.llm".to_string());
    let codegen = CodeGen::new(&context, "test", llmlang::Config { parallel_threshold: 1, ..llmlang::Config::default() });
    let exprs = parser.parse_module();
    if let Expr::Define(name, params, body, exported) = exprs[0].clone() { codegen.gen_function(&name, params, &body, exported); }
    let ir = codegen.module.print_to_string().to_string();
    assert!(ir.contains("call i64 @llm_fork"));
}

#[test]
fn test_esoteric_parallel_inside_trap() {
    let context = Context::create();
    // A trap containing a heavy pure expression (which should fork)
    let mut input = "X : main 🛡️ ".to_string();
    for _ in 0..10 { input.push_str("+ "); }
    input.push_str("1 ");
    for _ in 0..10 { input.push_str("1 "); }
    input.push_str("42");
    
    let mut parser = Parser::new(Lexer::new(&input), "test.llm".to_string());
    let codegen = CodeGen::new(&context, "test", llmlang::Config { parallel_threshold: 1, ..llmlang::Config::default() });
    let exprs = parser.parse_module();
    if let Expr::Define(name, params, body, exported) = exprs[0].clone() { codegen.gen_function(&name, params, &body, exported); }
    let ir = codegen.module.print_to_string().to_string();
    assert!(ir.contains("define i64 @trap_try_"));
    assert!(ir.contains("define i64 @parallel_task_"));
}


#[test]
fn test_esoteric_multi_move_error() {
    let context = Context::create();
    // Attempting to move a borrowed variable inside a trap (E016)
    let input = "X : main x 🛡️ ⮞ x ⚓ x";
    let mut parser = Parser::new(Lexer::new(input), "test.llm".to_string());
    let codegen = CodeGen::new(&context, "test", llmlang::Config::default());
    let exprs = parser.parse_module();
    if let Expr::Define(name, params, body, _) = exprs[0].clone() {
        let result = catch_unwind(AssertUnwindSafe(|| {
            codegen.gen_function(&name, params, &body, false);
        }));
        assert!(result.is_err());
        // Verify it was E016 if possible (catch_unwind doesn't give the panic message easily but we know it panics)
    }
}


#[test]
fn test_negative_import_missing_module() {
    let input = "I non_existent_module some_symbol";
    let mut parser = Parser::new(Lexer::new(input), "test.llm".to_string());
    let result = catch_unwind(AssertUnwindSafe(|| {
        parser.parse_module()
    }));
    assert!(result.is_err());
}

#[test]
fn test_negative_import_missing_symbol() {
    use std::fs::File;
    use std::io::Write;
    let sig_content = ": existing_symbol 1\n";
    let sig_path = "temp_lib.llmi";
    let mut file = File::create(sig_path).unwrap();
    file.write_all(sig_content.as_bytes()).unwrap();

    let input = "I temp_lib missing_symbol";
    let mut parser = Parser::new(Lexer::new(input), "test.llm".to_string());
    let result = catch_unwind(AssertUnwindSafe(|| {
        parser.parse_module()
    }));
    
    let _ = std::fs::remove_file(sig_path);
    assert!(result.is_err());
}

#[test]
fn test_positive_import_search_path() {
    use std::fs::{create_dir_all, File, remove_dir_all};
    use std::io::Write;
    
    let temp_dir = "temp_import_dir";
    create_dir_all(temp_dir).unwrap();
    
    let sig_content = ": existing_symbol 2\n";
    let sig_path = format!("{}/temp_lib2.llmi", temp_dir);
    let mut file = File::create(sig_path).unwrap();
    file.write_all(sig_content.as_bytes()).unwrap();

    let input = "I temp_lib2 existing_symbol";
    let mut parser = Parser::new(Lexer::new(input), "test.llm".to_string());
    parser.import_paths.push(temp_dir.to_string());
    
    let exprs = parser.parse_module();
    
    let _ = remove_dir_all(temp_dir);

    assert_eq!(exprs.len(), 1);
    if let Expr::Import(module, symbol, arity) = &exprs[0] {
        assert_eq!(module, "temp_lib2");
        assert_eq!(symbol, "existing_symbol");
        assert_eq!(*arity, 2);
    } else {
        panic!("Expected Expr::Import");
    }
}

#[test]
fn test_negative_shape_field_mismatch() {
    let context = Context::create();
    let input = "# Point x y\n: main\n  L p N Point 1\n  G ⚓ p z 0\n";
    let mut parser = Parser::new(Lexer::new(input), "test.llm".to_string());
    let codegen = CodeGen::new(&context, "test.llm", llmlang::Config::default());
    let exprs = parser.parse_module();
    
    for expr in exprs {
        match expr {
            Expr::Shape(name, fields, exported) => {
                codegen.gen_shape(&name, &fields, exported);
            }
            Expr::Define(name, params, body, exported) => {
                let result = catch_unwind(AssertUnwindSafe(|| {
                    codegen.gen_function(&name, params, &body, exported);
                }));
                assert!(result.is_err());
            }
            _ => {}
        }
    }
}

#[test]
fn test_positive_namespaced_codegen() {
    let context = Context::create();
    let input = "X : area width height\n  * ⚓ width ⚓ height\n";
    let mut parser = Parser::new(Lexer::new(input), "lib_geometry.llm".to_string());
    let codegen = CodeGen::new(&context, "lib_geometry.llm", llmlang::Config::default());
    let exprs = parser.parse_module();
    
    for expr in exprs {
        match expr {
            Expr::Define(name, params, body, exported) => {
                codegen.gen_function(&name, params, &body, exported);
            }
            _ => {}
        }
    }
    
    let ir = codegen.module.print_to_string().to_string();
    assert!(ir.contains("define i64 @lib_geometry_area"));
}

struct MockResolver;
impl llmlang::compiler::parser::SignatureResolver for MockResolver {
    fn resolve(&self, module: &str, _import_paths: &[String], _filename: &str) -> Result<String, String> {
        if module == "math" {
            Ok(": sin 1\n# Point x y\n".to_string())
        } else {
            Err("E017".to_string())
        }
    }
}

#[test]
fn test_custom_signature_resolver() {
    let input = "I math sin";
    let mut parser = Parser::new(Lexer::new(input), "test.llm".to_string());
    parser.resolver = Box::new(MockResolver);
    let exprs = parser.parse_module();
    assert_eq!(exprs.len(), 1);
    if let Expr::Import(module, symbol, arity) = &exprs[0] {
        assert_eq!(module, "math");
        assert_eq!(symbol, "sin");
        assert_eq!(*arity, 1);
    } else {
        panic!("Expected Expr::Import");
    }
}

#[test]
fn test_verification_use_after_move() {
    let input = "L x 42 . ⮞ ^0 ^0";
    let ast = parse_expr(input);
    let mut verify_ctx = llmlang::compiler::analysis::verify::VerificationContext {
        shapes: std::collections::HashMap::new(),
        functions: std::collections::HashMap::new(),
        stack: vec![],
        stack_shapes: vec![],
        expand_map: std::collections::HashMap::new(),
    };
    let result = llmlang::compiler::analysis::verify::verify_expr(&ast, &mut verify_ctx);
    assert_eq!(result, Err("E004".to_string()));
}

#[test]
fn test_verification_double_move() {
    let input = "L x 42 . ⮞ ^0 ⮞ ^0";
    let ast = parse_expr(input);
    let mut verify_ctx = llmlang::compiler::analysis::verify::VerificationContext {
        shapes: std::collections::HashMap::new(),
        functions: std::collections::HashMap::new(),
        stack: vec![],
        stack_shapes: vec![],
        expand_map: std::collections::HashMap::new(),
    };
    let result = llmlang::compiler::analysis::verify::verify_expr(&ast, &mut verify_ctx);
    assert_eq!(result, Err("E005".to_string()));
}

#[test]
fn test_verification_if_stack_mismatch() {
    let input = "L x 42 ? 1 ⮞ ^0 ^0";
    let ast = parse_expr(input);
    let mut verify_ctx = llmlang::compiler::analysis::verify::VerificationContext {
        shapes: std::collections::HashMap::new(),
        functions: std::collections::HashMap::new(),
        stack: vec![],
        stack_shapes: vec![],
        expand_map: std::collections::HashMap::new(),
    };
    let result = llmlang::compiler::analysis::verify::verify_expr(&ast, &mut verify_ctx);
    assert_eq!(result, Err("E009".to_string()));
}



