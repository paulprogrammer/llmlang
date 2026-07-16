//! Unified test harness core engine.
//!
//! Discovers functions tagged `M "test"` in a module's AST, isolates them into
//! a test execution tree, compiles them together with a synthesized dispatcher
//! `main`, and executes each test in a fresh sandboxed process. Results are
//! returned as an in-memory structured report consumed by both the CLI `test`
//! subcommand and the `run_symbol_tests` MCP tool.

use crate::compiler::ast::Expr;
use crate::compiler::lexer::Lexer;
use crate::compiler::parser::Parser;
use crate::Config;
use inkwell::context::Context;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Marker prefix for the result protocol line emitted by the generated harness main.
const RESULT_MARKER: &str = "__LLMTEST__:";
const RESULT_END_MARKER: &str = "__LLMTEST_END__";

pub const DEFAULT_TEST_DATA_DIR: &str = "./tests/data";

#[derive(Debug, Clone, Serialize)]
pub struct TestResult {
    pub name: String,
    pub passed: bool,
    /// Wall-clock execution time measured inside the sandbox via TAI64 `tns`.
    pub duration_ns: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub panic_message: Option<String>,
    /// SHA-256 of the test function body's structural fingerprint.
    pub fingerprint: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TestReport {
    pub file: String,
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub results: Vec<TestResult>,
}

impl TestReport {
    pub fn success(&self) -> bool {
        self.failed == 0
    }
}

/// A test function discovered in the AST via `M "test"` metadata.
#[derive(Debug)]
pub struct DiscoveredTest {
    pub name: String,
    pub fingerprint: String,
    pub define: Expr,
}

fn fingerprint_of(body: &Expr) -> String {
    let mut hasher = Sha256::new();
    hasher.update(body.structural_fingerprint());
    hex::encode(hasher.finalize())
}

/// Splits a parsed module into (test tree, production expressions).
///
/// Test-tagged definitions are isolated into the returned test list and
/// stripped from the production expression list. The parser already rejects
/// `M "test"` metadata whose target is not a function definition (E019).
pub fn discover_tests(expressions: Vec<Expr>) -> Result<(Vec<DiscoveredTest>, Vec<Expr>), String> {
    let mut tests = Vec::new();
    let mut production = Vec::new();

    for expr in expressions {
        let mut is_test = false;
        if let Expr::Metadata(tag, _, target) = &expr {
            if matches!(&**tag, Expr::String(s) if s == "test") {
                let mut inner = &**target;
                while let Expr::Metadata(_, _, t) = inner {
                    inner = t;
                }
                if let Expr::Define(name, params, body, _) = inner {
                    if !params.is_empty() {
                        return Err(format!(
                            "E019: test function '{}' must take no parameters",
                            name
                        ));
                    }
                    tests.push(DiscoveredTest {
                        name: name.clone(),
                        fingerprint: fingerprint_of(body),
                        define: inner.clone(),
                    });
                    is_test = true;
                } else {
                    return Err("E019: M \"test\" metadata must target a function definition (:)".to_string());
                }
            }
        }
        if !is_test {
            production.push(expr);
        }
    }
    Ok((tests, production))
}

/// Generates the llmlang source of the dispatcher `main`.
///
/// The binary selects a single test via `env "LLM_TEST_NAME"`, wraps its
/// execution in a trap (`^`) so panics are recorded rather than crashing the
/// runner, measures execution with `tns`, and prints a machine-readable
/// protocol block to stdout:
///
/// `__LLMTEST__:<1|0>:<duration_ns>:<panic_msg>__LLMTEST_END__`
fn generate_harness_main(test_names: &[String]) -> String {
    let mut src = String::from(": main\nL tname env \"LLM_TEST_NAME\"\n");
    for name in test_names {
        src.push_str(&format!("? sr $ tname \"^{}$\"\n", name));
        src.push_str(&format!(
            "L t0 tns\n\
             L ok ^ . @0 {} 1 0\n\
             L t1 tns\n\
             . ) 1 sc \"\\n{}\" sc str $ ok sc \":\" sc str - $ t1 $ t0 sc \":\" sc env \"LLM_PANIC_MSG\" \"{}\\n\"\n\
             ? = $ ok 1 0 1\n",
            name, RESULT_MARKER, RESULT_END_MARKER
        ));
    }
    src.push_str(". ) 2 \"unknown test name\\n\" 1\n");
    src
}

/// Locates the `llm-clang` linker wrapper. Search order: `LLM_CLANG` env var,
/// ancestors of the compiler binary, ancestors of the source file, cwd.
fn find_llm_clang(source_path: &Path) -> Result<PathBuf, String> {
    if let Ok(p) = std::env::var("LLM_CLANG") {
        let pb = PathBuf::from(p);
        if pb.is_file() {
            return Ok(pb);
        }
    }
    let mut roots: Vec<PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        roots.extend(exe.ancestors().skip(1).map(Path::to_path_buf));
    }
    if let Ok(abs) = std::fs::canonicalize(source_path) {
        roots.extend(abs.ancestors().skip(1).map(Path::to_path_buf));
    }
    if let Ok(cwd) = std::env::current_dir() {
        roots.extend(cwd.ancestors().map(Path::to_path_buf));
    }
    for root in roots {
        let cand = root.join("llm-clang");
        if cand.is_file() {
            return Ok(cand);
        }
    }
    Err("could not locate llm-clang (set LLM_CLANG to its path)".to_string())
}

fn parse_module(source_path: &str, import_paths: &[String]) -> Result<Vec<Expr>, String> {
    let input = std::fs::read_to_string(source_path)
        .map_err(|e| format!("E999: Could not read file {}: {}", source_path, e))?;
    let lexer = Lexer::new(&input);
    let mut parser = Parser::new(lexer, source_path.to_string()).map_err(|e| e.to_string())?;
    parser.import_paths = import_paths.to_vec();
    if let Some(parent) = Path::new(source_path).parent() {
        parser.import_paths.push(parent.display().to_string());
    }
    parser.parse_module().map_err(|e| e.to_string())
}

/// Compiles the isolated test tree plus its module context into a runnable
/// harness binary. Returns the binary path.
fn compile_harness(
    source_path: &str,
    production: &[Expr],
    tests: &[DiscoveredTest],
    import_paths: &[String],
    work_dir: &Path,
) -> Result<PathBuf, String> {
    // Assemble the harness module: production code (minus any existing main),
    // the isolated test functions, and the synthesized dispatcher main.
    let mut exprs: Vec<Expr> = Vec::new();
    for expr in production {
        let inner = {
            let mut e = expr;
            while let Expr::Metadata(_, _, t) = e {
                e = t;
            }
            e
        };
        if matches!(inner, Expr::Define(name, _, _, _) if name == "main") {
            continue;
        }
        exprs.push(expr.clone());
    }
    for t in tests {
        exprs.push(t.define.clone());
    }

    let names: Vec<String> = tests.iter().map(|t| t.name.clone()).collect();
    let main_src = generate_harness_main(&names);
    let mut main_parser = Parser::new(Lexer::new(&main_src), "<llmtest-harness>".to_string())
        .map_err(|e| format!("harness generation error: {}", e))?;
    let main_exprs = main_parser
        .parse_module()
        .map_err(|e| format!("harness generation error: {}", e))?;
    exprs.extend(main_exprs);

    // Affine/ownership verification runs over the full module including the
    // test tree; the sandbox does not bypass consume-once constraints.
    crate::compiler::analysis::verify::verify_module(&exprs, source_path)
        .map_err(|e| e.to_string())?;

    let context = Context::create();
    let codegen = crate::compiler::codegen::CodeGen::new(&context, source_path, Config::default());
    codegen.analyze_module_types(&exprs);
    for expr in exprs {
        match expr {
            Expr::Shape(name, fields, exported) => {
                codegen.gen_shape(&name, &fields, exported);
            }
            Expr::Define(name, params, body, exported) => {
                codegen
                    .gen_function(&name, params, &body, exported, None)
                    .map_err(|e| e.to_string())?;
            }
            Expr::Metadata(tag, val, target) => {
                if let (Expr::String(tag_str), Expr::String(span_name)) = (&*tag, &*val) {
                    if tag_str == "otel" {
                        if let Expr::Define(name, params, body, exported) = &*target {
                            codegen
                                .gen_function(name, params.clone(), body, *exported, Some(span_name.clone()))
                                .map_err(|e| e.to_string())?;
                        }
                    }
                }
            }
            Expr::Import(module, symbol, arity) => {
                codegen.gen_import(&module, &symbol, arity);
            }
            _ => {}
        }
    }

    let obj_path = work_dir.join("llmtest_harness.o");
    let bin_path = work_dir.join("llmtest_harness_bin");
    codegen
        .emit_to_file(&obj_path.display().to_string())
        .map_err(|e| format!("object emission failed: {}", e))?;

    let llm_clang = find_llm_clang(Path::new(source_path))?;
    let mut cmd = Command::new(&llm_clang);
    cmd.arg(&obj_path);
    for p in import_paths {
        cmd.arg("-I").arg(p);
    }
    cmd.arg("-o").arg(&bin_path);
    let out = cmd
        .output()
        .map_err(|e| format!("failed to invoke {}: {}", llm_clang.display(), e))?;
    if !out.status.success() {
        return Err(format!(
            "link failed:\n{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(bin_path)
}

/// Runs one test in a fresh process. Memory context and environment are
/// re-initialized per test: each execution is a new OS process with a
/// controlled environment, so no state leaks between tests.
fn run_single_test(bin: &Path, test: &DiscoveredTest, test_data_dir: &str) -> TestResult {
    let output = Command::new(bin)
        .env("LLM_TEST_NAME", &test.name)
        .env("TEST_DATA_DIR", test_data_dir)
        .env_remove("LLM_PANIC_MSG")
        .output();

    let output = match output {
        Ok(o) => o,
        Err(e) => {
            return TestResult {
                name: test.name.clone(),
                passed: false,
                duration_ns: 0,
                panic_message: Some(format!("failed to spawn test process: {}", e)),
                fingerprint: test.fingerprint.clone(),
            };
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    // The protocol block is the last marker occurrence; test code may write
    // freely to stdout before it.
    let parsed = stdout.rfind(RESULT_MARKER).and_then(|start| {
        let payload = &stdout[start + RESULT_MARKER.len()..];
        let payload = &payload[..payload.find(RESULT_END_MARKER)?];
        let mut parts = payload.splitn(3, ':');
        let ok = parts.next()?.trim() == "1";
        let duration_ns: i64 = parts.next()?.trim().parse().ok()?;
        let msg = parts.next().unwrap_or("").to_string();
        Some((ok, duration_ns, msg))
    });

    match parsed {
        Some((passed, duration_ns, msg)) => TestResult {
            name: test.name.clone(),
            passed,
            duration_ns,
            panic_message: if passed || msg.is_empty() { None } else { Some(msg) },
            fingerprint: test.fingerprint.clone(),
        },
        None => TestResult {
            name: test.name.clone(),
            passed: false,
            duration_ns: 0,
            panic_message: Some(format!(
                "test process terminated without result (exit: {:?}): {}",
                output.status.code(),
                String::from_utf8_lossy(&output.stderr).trim()
            )),
            fingerprint: test.fingerprint.clone(),
        },
    }
}

/// Core engine entry point: discovers, compiles, and executes all tests in a
/// single `.llm` file, returning the structured in-memory report.
pub fn run_tests(
    source_path: &str,
    test_data_dir: Option<&str>,
    import_paths: &[String],
) -> Result<TestReport, String> {
    let expressions = parse_module(source_path, import_paths)?;
    let (tests, production) = discover_tests(expressions)?;

    let mut report = TestReport {
        file: source_path.to_string(),
        total: tests.len(),
        passed: 0,
        failed: 0,
        results: Vec::new(),
    };
    if tests.is_empty() {
        return Ok(report);
    }

    let data_dir = test_data_dir.unwrap_or(DEFAULT_TEST_DATA_DIR);
    // Resolve to an absolute path so tests are immune to cwd differences.
    let data_dir = std::fs::canonicalize(data_dir)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| data_dir.to_string());

    let work_dir = std::env::temp_dir().join(format!("llmtest_{}", std::process::id()));
    std::fs::create_dir_all(&work_dir).map_err(|e| format!("cannot create work dir: {}", e))?;

    let result = (|| {
        let bin = compile_harness(source_path, &production, &tests, import_paths, &work_dir)?;
        for test in &tests {
            let res = run_single_test(&bin, test, &data_dir);
            if res.passed {
                report.passed += 1;
            } else {
                report.failed += 1;
            }
            report.results.push(res);
        }
        Ok(report)
    })();

    let _ = std::fs::remove_dir_all(&work_dir);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(src: &str) -> Vec<Expr> {
        let mut parser = Parser::new(Lexer::new(src), "<test>".to_string()).unwrap();
        parser.parse_module().unwrap()
    }

    #[test]
    fn discovers_tagged_tests_and_strips_them_from_production() {
        let exprs = parse(
            ": helper x + $ x 1\n\
             M \"test\" \"arith\" : test_add ? = + 2 3 5 0 ! \"broken\"\n\
             : main 0\n",
        );
        let (tests, production) = discover_tests(exprs).unwrap();
        assert_eq!(tests.len(), 1);
        assert_eq!(tests[0].name, "test_add");
        assert!(!tests[0].fingerprint.is_empty());
        // Production tree keeps helper + main, loses the test.
        assert_eq!(production.len(), 2);
    }

    #[test]
    fn empty_suite_discovers_zero_tests() {
        let exprs = parse(": main 0\n");
        let (tests, production) = discover_tests(exprs).unwrap();
        assert!(tests.is_empty());
        assert_eq!(production.len(), 1);
    }

    #[test]
    fn malformed_test_metadata_is_a_parse_error() {
        let mut parser = Parser::new(
            Lexer::new("M \"test\" \"bad\" 42\n"),
            "<test>".to_string(),
        )
        .unwrap();
        let err = parser.parse_module().unwrap_err();
        assert_eq!(err.code, "E019");
    }

    #[test]
    fn test_with_parameters_is_rejected() {
        let exprs = parse("M \"test\" \"bad\" : test_param x $ x\n");
        let err = discover_tests(exprs).unwrap_err();
        assert!(err.contains("must take no parameters"));
    }

    #[test]
    fn harness_main_parses_and_dispatches_all_tests() {
        let names = vec!["test_a".to_string(), "test_b".to_string()];
        let src = generate_harness_main(&names);
        let mut parser = Parser::new(Lexer::new(&src), "<harness>".to_string()).unwrap();
        let exprs = parser.parse_module().unwrap();
        assert_eq!(exprs.len(), 1);
        match &exprs[0] {
            Expr::Define(name, params, _, _) => {
                assert_eq!(name, "main");
                assert!(params.is_empty());
            }
            other => panic!("expected main define, got {:?}", other),
        }
    }
}
