use llmlang::lexer::Lexer;
use llmlang::parser::{Parser, Expr};
use llmlang::codegen::CodeGen;
use inkwell::context::Context;
use std::env;
use std::fs;
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();
    
    if args.len() < 2 {
        eprintln!("Usage: {} <file_path>", args[0]);
        process::exit(1);
    }

    let file_path = &args[1];
    let input = fs::read_to_string(file_path).unwrap_or_else(|_| {
        eprintln!("E999: Could not read file {}", file_path);
        process::exit(1);
    });

    let context = Context::create();
    let codegen = CodeGen::new(&context, file_path);

    let lexer = Lexer::new(&input);
    let mut parser = Parser::new(lexer);
    
    // Catch-all for parsing/codegen errors
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let expressions = parser.parse_module();
        
        for expr in expressions {
            match expr {
                Expr::Shape(name, fields) => {
                    codegen.gen_shape(&name, &fields);
                }
                Expr::Define(name, params, body) => {
                    codegen.gen_function(&name, params, &body);
                }
                _ => {
                    // Ignore or wrap in main? For now, we only allow Shapes and Defines at top level.
                }
            }
        }
    }));

    if result.is_err() {
        // Human error code mapping is in DIAGNOSTICS.md
        // CLI only prints the code to stay token-efficient.
        process::exit(1);
    }

    println!("{}", codegen.module.print_to_string().to_string());
}
