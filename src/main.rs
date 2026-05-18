use llmlang::lexer::{Lexer};
use llmlang::parser::{Parser, Expr};
use llmlang::codegen::CodeGen;
use inkwell::context::Context;

fn main() {
    let context = Context::create();

    println!("--- Test 1: Valid Move ---");
    let input1 = ": add_one x + > ^0 1";
    let ast1 = Parser::new(Lexer::new(input1)).parse_expr();
    let codegen1 = CodeGen::new(&context, "test1");
    if let Expr::Define(name, args, body) = ast1 {
        codegen1.gen_function(&name, args.len(), &body);
        println!("Success!\n");
    }

    println!("--- Test 2: Variable Leak (Warning) ---");
    let input2 = ": leak x 42"; 
    let ast2 = Parser::new(Lexer::new(input2)).parse_expr();
    let codegen2 = CodeGen::new(&context, "test2");
    if let Expr::Define(name, args, body) = ast2 {
        codegen2.gen_function(&name, args.len(), &body);
        println!("Note: Warning above is expected.\n");
    }

    println!("--- Test 3: Double Move (Panic) ---");
    let input3 = ": double_move x + > ^0 > ^0"; 
    let ast3 = Parser::new(Lexer::new(input3)).parse_expr();
    let codegen3 = CodeGen::new(&context, "test3");
    if let Expr::Define(name, args, body) = ast3 {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            codegen3.gen_function(&name, args.len(), &body);
        }));
        if result.is_err() {
            println!("Caught expected panic from double move!\n");
        }
    }
}
