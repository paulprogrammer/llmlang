use llmlang::compiler::lexer::Lexer;
use llmlang::compiler::parser::Parser;
use llmlang::compiler::ast::Expr;
use llmlang::compiler::codegen::CodeGen;
use inkwell::context::Context;
use std::env;
use std::fs;
use std::process;

fn print_help() {
    println!("\
llmlang v{}
The Turing-complete, polymorphic language optimized for LLM token usage and execution speed.

USAGE:
    llmlang <INPUT> [OPTIONS]

ARGS:
    <INPUT>             Path to the .llm source file

OPTIONS:
    -o <OUTPUT>         Path to the output file (object file by default)
    -S, --emit-ir       Emit LLVM IR to stdout or to the file specified by -o
    --emit-sig          Emit structural signature file (.llms) for indexing
    -h, --help          Print help information
    -V, --version       Print version information
", env!("CARGO_PKG_VERSION"));
}

fn print_version() {
    let llvm_version = inkwell::support::get_llvm_version();
    println!("llmlang {}", env!("CARGO_PKG_VERSION"));
    println!("Build Options:");
    println!("  LLVM Version: {}.{}.{}", llvm_version.0, llvm_version.1, llvm_version.2);
    println!("  Targets: all, webassembly");
    println!("  Ownership: linear state checking enabled");
    println!("  Optimization: monomorphization expansion enabled");
}

fn main() {
    let args: Vec<String> = env::args().collect();
    
    if args.len() < 2 {
        print_help();
        process::exit(1);
    }

    let mut input_path = None;
    let mut output_path = None;
    let mut emit_ir = false;
    let mut emit_sig = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => {
                print_help();
                process::exit(0);
            }
            "-V" | "--version" => {
                print_version();
                process::exit(0);
            }
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
            _ => {
                eprintln!("Unknown argument: {}", args[i]);
                process::exit(1);
            }
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
    let mut parser = Parser::new(lexer, input_path.to_string());
    
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
