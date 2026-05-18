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
        eprintln!("Usage: {} <input.llm> [-o <output>] [--emit-ir]", args[0]);
        process::exit(1);
    }

    let mut input_path = None;
    let mut output_path = None;
    let mut emit_ir = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-o" => {
                i += 1;
                if i < args.len() {
                    output_path = Some(&args[i]);
                }
            }
            "--emit-ir" | "-S" => {
                emit_ir = true;
            }
            path if !path.starts_with('-') => {
                input_path = Some(path);
            }
            _ => {}
        }
        i += 1;
    }

    let input_path = input_path.expect("E998: No input file specified");
    let input = fs::read_to_string(input_path).unwrap_or_else(|_| {
        eprintln!("E999: Could not read file {}", input_path);
        process::exit(1);
    });

    let context = Context::create();
    let codegen = CodeGen::new(&context, input_path);

    let lexer = Lexer::new(&input);
    let mut parser = Parser::new(lexer);
    
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
                _ => {}
            }
        }
    }));

    if result.is_err() {
        process::exit(1);
    }

    if emit_ir || output_path.is_none() {
        let ir = codegen.module.print_to_string().to_string();
        if let Some(out) = output_path {
            fs::write(out, ir).expect("Could not write IR to file");
        } else {
            println!("{}", ir);
        }
    } else {
        let out = output_path.unwrap();
        codegen.emit_to_file(out).expect("E997: Object emission failed");
    }
}
