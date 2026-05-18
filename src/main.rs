use llmlang::lexer::Lexer;
use llmlang::parser::{Parser, Expr};
use llmlang::codegen::CodeGen;
use inkwell::context::Context;
use std::env;
use std::fs;
use std::process;
use std::collections::HashMap;

fn main() {
    let args: Vec<String> = env::args().collect();
    
    if args.len() < 2 {
        eprintln!("Usage: {} <input.llm> [-o <output>] [--emit-ir] [--emit-sig]", args[0]);
        process::exit(1);
    }

    let mut input_path = None;
    let mut output_path = None;
    let mut emit_ir = false;
    let mut emit_sig = false;

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
            "--emit-sig" => {
                emit_sig = true;
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
                Expr::Shape(name, fields, _exported) => {
                    codegen.gen_shape(&name, &fields);
                }
                Expr::Define(name, params, body, _exported) => {
                    codegen.gen_function(&name, params, &body);
                }
                Expr::Import(module, symbol) => {
                    codegen.gen_import(&module, &symbol);
                }
                _ => {}
            }
        }
    }));

    if result.is_err() {
        process::exit(1);
    }

    if emit_sig {
        let sig = codegen.emit_signature_file();
        let sig_path = match output_path {
            Some(p) => format!("{}.llms", p),
            None => format!("{}.llms", input_path),
        };
        fs::write(sig_path, sig).expect("Could not write signature file");
    }

    if emit_ir || (!emit_sig && output_path.is_none()) {
        let ir = codegen.module.print_to_string().to_string();
        if let Some(out) = output_path {
            fs::write(out, ir).expect("Could not write IR to file");
        } else {
            println!("{}", ir);
        }
    } else if let Some(out) = output_path {
        codegen.emit_to_file(out).expect("E997: Object emission failed");
    }
}
