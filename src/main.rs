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
    -c, --config <FILE> Path to a JSON configuration file
    -I <PATH>           Add a directory to the module search path
    --parallel <NUM>    Set the complexity threshold for auto-parallelism (default: 50)
    --threads <NUM>     Set the number of worker threads (default: 8)
    --queue <NUM>       Set the thread pool queue size (default: 64)
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
    let mut config = llmlang::Config::default();
    let mut import_paths = Vec::new();

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
            "-c" | "--config" => {
                i += 1;
                if i < args.len() {
                    let config_str = fs::read_to_string(&args[i]).unwrap_or_else(|_| {
                        eprintln!("Could not read config file {}", args[i]);
                        process::exit(1);
                    });
                    config = serde_json::from_str(&config_str).unwrap_or_else(|e| {
                        eprintln!("Could not parse config file {}: {}", args[i], e);
                        process::exit(1);
                    });
                }
            }
            "-I" => {
                i += 1;
                if i < args.len() {
                    import_paths.push(args[i].clone());
                }
            }
            "--parallel" => {
                i += 1;
                if i < args.len() {
                    config.parallel_threshold = args[i].parse().unwrap_or(50);
                }
            }
            "--threads" => {
                i += 1;
                if i < args.len() {
                    config.max_threads = args[i].parse().unwrap_or(8);
                }
            }
            "--queue" => {
                i += 1;
                if i < args.len() {
                    config.queue_size = args[i].parse().unwrap_or(64);
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

    let input_path_str = input_path.expect("E998: No input file specified");
    let input = fs::read_to_string(input_path_str).unwrap_or_else(|_| {
        eprintln!("E999: Could not read file {}", input_path_str);
        process::exit(1);
    });

    let context = Context::create();
    let codegen = CodeGen::new(&context, input_path_str, config);

    let lexer = Lexer::new(&input);
    let mut parser = match Parser::new(lexer, input_path_str.to_string()) {
        Ok(p) => p,
        Err(err) => {
            eprintln!("{}", err);
            process::exit(1);
        }
    };
    parser.import_paths = import_paths;
    
    let expressions = match parser.parse_module() {
        Ok(exprs) => exprs,
        Err(err) => {
            eprintln!("{}", err);
            process::exit(1);
        }
    };

    // 1. Dead Function Elimination (DFE)
    let expressions = llmlang::compiler::analysis::prune_dead_code(expressions);

    // 2. Semantic Verification Pass
    if let Err(err_code) = llmlang::compiler::analysis::verify::verify_module(&expressions, input_path_str) {
        eprintln!("{}", err_code);
        process::exit(1);
    }

    codegen.analyze_module_types(&expressions);

    for expr in expressions {
        match expr {
            Expr::Shape(name, fields, exported) => {
                codegen.gen_shape(&name, &fields, exported);
            }
            Expr::Define(name, params, body, exported) => {
                if let Err(err) = codegen.gen_function(&name, params, &body, exported) {
                    eprintln!("{}", err);
                    process::exit(1);
                }
            }
            Expr::Import(module, symbol, arity) => {
                codegen.gen_import(&module, &symbol, arity);
            }
            _ => {}
        }
    }

    if emit_sig || (output_path.is_some() && codegen.has_exports.get()) {
        let sig = codegen.emit_signature_file();
        let sig_path = match output_path {
            Some(p) => {
                if p.ends_with(".o") {
                    format!("{}.llmi", &p[..p.len()-2])
                } else {
                    format!("{}.llmi", p)
                }
            },
            None => format!("{}.llmi", input_path_str),
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
