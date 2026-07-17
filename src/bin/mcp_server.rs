use async_trait::async_trait;
use mcp_sdk_rs::{
    error::{Error, ErrorCode},
    server::{Server, ServerHandler},
    transport::{stdio::StdioTransport},
    types::{ClientCapabilities, Implementation, ServerCapabilities, MessageContent},
};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use std::collections::HashMap;
use walkdir::WalkDir;
use llmlang::compiler::lexer::Lexer;
use llmlang::compiler::parser::Parser;
use llmlang::compiler::ast::Expr;
use sha2::{Sha256, Digest};
use tokio::io::{stdin, stdout, AsyncBufReadExt, AsyncWriteExt, BufReader};

const LLM_SPEC: &str = include_str!("../../LLM_SPEC.md");
const MCP_GUIDE: &str = include_str!("../../MCP_GUIDE.md");

#[derive(Debug)]
struct SymbolMetadata {
    expr: Expr,
    path: String,
    line: usize,
    calls: Vec<String>,
    fingerprint: String,
}

struct CodebaseIndex {
    // A name can be defined in several files, so each name maps to one
    // entry per defining file. Calls and fingerprints live on the entry
    // itself: re-indexing a file replaces its entries wholesale, so the
    // index cannot accumulate stale or duplicate data.
    functions: HashMap<String, Vec<SymbolMetadata>>,
    shapes: HashMap<String, (Vec<String>, String, usize)>,
}

fn paths_match(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    match (std::path::Path::new(a).canonicalize(), std::path::Path::new(b).canonicalize()) {
        (Ok(ca), Ok(cb)) => ca == cb,
        _ => false,
    }
}

impl CodebaseIndex {
    fn new() -> Self {
        Self {
            functions: HashMap::new(),
            shapes: HashMap::new(),
        }
    }

    /// Drop everything previously indexed from `path` so re-analysis
    /// replaces a file's entries instead of appending to them.
    fn purge_file(&mut self, path: &str) {
        for metas in self.functions.values_mut() {
            metas.retain(|m| m.path != path);
        }
        self.functions.retain(|_, metas| !metas.is_empty());
        self.shapes.retain(|_, (_, p, _)| p != path);
    }

    fn index_exprs(&mut self, path: &str, expressions: Vec<Expr>) -> usize {
        self.purge_file(path);
        let mut count = 0;
        for expr in expressions {
            match expr {
                Expr::Define(name, _, body, _) => {
                    let calls = body.get_calls();
                    let fp = body.structural_fingerprint();
                    let mut hasher = Sha256::new();
                    hasher.update(&fp);
                    let hash = hex::encode(hasher.finalize());

                    self.functions.entry(name).or_default().push(SymbolMetadata {
                        expr: *body,
                        path: path.to_string(),
                        line: 0, // Rough estimation or we'd need AST pos
                        calls,
                        fingerprint: hash,
                    });
                    count += 1;
                }
                Expr::Shape(name, fields, _) => {
                    self.shapes.insert(name, (fields, path.to_string(), 0));
                }
                _ => {}
            }
        }
        count
    }

    /// Resolve a function name to a single definition. `path` disambiguates
    /// when the same name is defined in more than one indexed file.
    fn resolve_function(&self, name: &str, path: Option<&str>) -> Result<&SymbolMetadata, String> {
        let metas = self
            .functions
            .get(name)
            .filter(|m| !m.is_empty())
            .ok_or_else(|| format!("Function not found: {}", name))?;
        let known_paths: Vec<&str> = metas.iter().map(|m| m.path.as_str()).collect();
        match path {
            Some(p) => metas.iter().find(|m| paths_match(&m.path, p)).ok_or_else(|| {
                format!(
                    "Function '{}' is not defined in '{}'; known locations: {:?}",
                    name, p, known_paths
                )
            }),
            None if metas.len() == 1 => Ok(&metas[0]),
            None => Err(format!(
                "Function '{}' is defined in multiple files: {:?}; pass 'path' to disambiguate",
                name, known_paths
            )),
        }
    }
}

struct LLMLangMCPHandler {
    index: RwLock<CodebaseIndex>,
}

impl LLMLangMCPHandler {
    fn new() -> Self {
        Self {
            index: RwLock::new(CodebaseIndex::new()),
        }
    }

    async fn analyze_path(&self, path: &str) -> Result<String, String> {
        let mut index = self.index.write().await;
        let mut count = 0;

        for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
            if entry.path().extension().and_then(|s| s.to_str()) == Some("llm") {
                let content = match std::fs::read_to_string(entry.path()) {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("Failed to read {}: {}", entry.path().display(), e);
                        continue;
                    }
                };
                let mut parser = match Parser::new(Lexer::new(&content), entry.path().display().to_string()) {
                    Ok(p) => p,
                    Err(e) => {
                        eprintln!("Failed to initialize parser for {}: {}", entry.path().display(), e);
                        continue;
                    }
                };
                let expressions = match parser.parse_module() {
                    Ok(exprs) => exprs,
                    Err(e) => {
                        eprintln!("Failed to parse {}: {}", entry.path().display(), e);
                        continue;
                    }
                };

                count += index.index_exprs(&entry.path().display().to_string(), expressions);
            }
        }
        Ok(format!("Indexed {} functions and {} shapes", count, index.shapes.len()))
    }
}

#[async_trait]
impl ServerHandler for LLMLangMCPHandler {
    async fn initialize(
        &self,
        _implementation: Implementation,
        _capabilities: ClientCapabilities,
    ) -> Result<ServerCapabilities, Error> {
        let mut caps = ServerCapabilities::default();
        caps.tools = Some(json!({}));
        caps.resources = Some(json!({}));
        Ok(caps)
    }

    async fn handle_method(&self, method: &str, params: Option<Value>) -> Result<Value, Error> {
        match method {
            "resources/list" => {
                let resources = json!([
                    {
                        "uri": "llm://spec",
                        "name": "llmlang Specification",
                        "mimeType": "text/markdown",
                        "description": "Token-by-token grammar, operator specification, memory safety rules, and canonical patterns"
                    },
                    {
                        "uri": "llm://agent-workflow",
                        "name": "MCP Agent Workflow Guide",
                        "mimeType": "text/markdown",
                        "description": "MCP server capabilities, tools, and strategic workflows for codebase analysis"
                    }
                ]);
                Ok(json!({ "resources": resources }))
            }
            "resources/read" => {
                let params = params.ok_or_else(|| Error::protocol(ErrorCode::InvalidParams, "Missing params"))?;
                let uri = params.get("uri").and_then(|v| v.as_str()).ok_or_else(|| Error::protocol(ErrorCode::InvalidParams, "Missing uri"))?;
                
                let content = match uri {
                    "llm://spec" => LLM_SPEC,
                    "llm://fundamentals" => LLM_SPEC,
                    "llm://agent-workflow" => MCP_GUIDE,
                    _ => return Err(Error::protocol(ErrorCode::InvalidParams, format!("Unknown resource: {}", uri))),
                };

                Ok(json!({
                    "contents": [
                        {
                            "uri": uri,
                            "mimeType": "text/markdown",
                            "text": content
                        }
                    ]
                }))
            }
            "tools/list" => {
                let tools = json!([
                    {
                        "name": "analyze_codebase",
                        "description": "Indexes all .llm files in the given path",
                        "input_schema": {
                            "type": "object",
                            "properties": {
                                "path": { "type": "string", "description": "Local path to the codebase" }
                            },
                            "required": ["path"]
                        }
                    },
                    {
                        "name": "search_symbols",
                        "description": "Searches for functions or shapes by name",
                        "input_schema": {
                            "type": "object",
                            "properties": {
                                "query": { "type": "string" }
                            },
                            "required": ["query"]
                        }
                    },
                    {
                        "name": "get_definition",
                        "description": "Returns the AST and location of a function or shape",
                        "input_schema": {
                            "type": "object",
                            "properties": {
                                "name": { "type": "string" }
                            },
                            "required": ["name"]
                        }
                    },
                    {
                        "name": "get_diagnostics",
                        "description": "Runs parser and analysis on a file and returns errors",
                        "input_schema": {
                            "type": "object",
                            "properties": {
                                "path": { "type": "string" }
                            },
                            "required": ["path"]
                        }
                    },
                    {
                        "name": "find_callers",
                        "description": "Finds all functions that call the given symbol",
                        "input_schema": {
                            "type": "object",
                            "properties": {
                                "symbol": { "type": "string" }
                            },
                            "required": ["symbol"]
                        }
                    },
                    {
                        "name": "structural_search",
                        "description": "Finds functions with a similar AST structure",
                        "input_schema": {
                            "type": "object",
                            "properties": {
                                "function_name": { "type": "string" }
                            },
                            "required": ["function_name"]
                        }
                    },
                    {
                        "name": "run_symbol_tests",
                        "description": "Discovers functions tagged M \"test\" in a .llm file, executes each in an isolated sandboxed process, and returns structured JSON mapping test failures to the target symbol's AST fingerprint",
                        "input_schema": {
                            "type": "object",
                            "properties": {
                                "path": { "type": "string", "description": "Path to the .llm source file containing the tests" },
                                "test_data_dir": { "type": "string", "description": "Base path for external mock data, injected as TEST_DATA_DIR (default ./tests/data)" }
                            },
                            "required": ["path"]
                        }
                    },
                    {
                        "name": "patch_symbol",
                        "description": "Replaces a function's body AST and rewrites the source file",
                        "input_schema": {
                            "type": "object",
                            "properties": {
                                "function_name": { "type": "string", "description": "The name of the function to patch" },
                                "new_body_ast": { "type": "object", "description": "The new AST structure for the function body (JSON format matching Expr struct)" },
                                "path": { "type": "string", "description": "Source file defining the function; required when the name is defined in more than one indexed file" }
                            },
                            "required": ["function_name", "new_body_ast"]
                        }
                    }
                ]);
                Ok(json!({ "tools": tools }))
            }
            "tools/call" => {
                let params = params.ok_or_else(|| Error::protocol(ErrorCode::InvalidParams, "Missing params"))?;
                let name = params.get("name").and_then(|v| v.as_str()).ok_or_else(|| Error::protocol(ErrorCode::InvalidParams, "Missing tool name"))?;
                let args = params.get("arguments").ok_or_else(|| Error::protocol(ErrorCode::InvalidParams, "Missing arguments"))?;

                match name {
                    "analyze_codebase" => {
                        let path = args.get("path").and_then(|v| v.as_str()).ok_or_else(|| Error::protocol(ErrorCode::InvalidParams, "Missing path"))?;
                        let res = self.analyze_path(path).await.map_err(|e| Error::protocol(ErrorCode::InternalError, e))?;
                        Ok(json!({
                            "content": [MessageContent::Text { text: res }]
                        }))
                    }
                    "search_symbols" => {
                        let query = args.get("query").and_then(|v| v.as_str()).ok_or_else(|| Error::protocol(ErrorCode::InvalidParams, "Missing query"))?;
                        let index = self.index.read().await;
                        let mut results = Vec::new();
                        for name in index.functions.keys() {
                            if name.contains(query) {
                                results.push(format!("Function: {}", name));
                            }
                        }
                        for name in index.shapes.keys() {
                            if name.contains(query) {
                                results.push(format!("Shape: {}", name));
                            }
                        }
                        Ok(json!({
                            "content": [MessageContent::Text { text: results.join("\n") }]
                        }))
                    }
                    "get_definition" => {
                        let name = args.get("name").and_then(|v| v.as_str()).ok_or_else(|| Error::protocol(ErrorCode::InvalidParams, "Missing name"))?;
                        let index = self.index.read().await;
                        if let Some(metas) = index.functions.get(name) {
                            let text = metas.iter()
                                .map(|meta| format!("Function: {}\nPath: {}\nLine: {}\nAST: {:?}", name, meta.path, meta.line, meta.expr))
                                .collect::<Vec<_>>()
                                .join("\n---\n");
                            Ok(json!({
                                "content": [MessageContent::Text { text }]
                            }))
                        } else if let Some((fields, path, line)) = index.shapes.get(name) {
                            Ok(json!({
                                "content": [MessageContent::Text { 
                                    text: format!("Shape: {}\nPath: {}\nLine: {}\nFields: {:?}", name, path, line, fields) 
                                }]
                            }))
                        } else {
                            Err(Error::protocol(ErrorCode::InvalidParams, "Symbol not found"))
                        }
                    }
                    "get_diagnostics" => {
                        let path = args.get("path").and_then(|v| v.as_str()).ok_or_else(|| Error::protocol(ErrorCode::InvalidParams, "Missing path"))?;
                        let content = std::fs::read_to_string(path).map_err(|e| Error::protocol(ErrorCode::InternalError, e.to_string()))?;
                        let mut parser = match Parser::new(Lexer::new(&content), path.to_string()) {
                            Ok(p) => p,
                            Err(e) => {
                                return Ok(json!({
                                    "content": [MessageContent::Text { text: format!("Error: {}", e) }]
                                }));
                            }
                        };
                        
                        match parser.parse_module() {
                            Ok(_) => Ok(json!({
                                "content": [MessageContent::Text { text: "No errors found".to_string() }]
                            })),
                            Err(e) => {
                                Ok(json!({
                                    "content": [MessageContent::Text { text: format!("Error: {}", e) }]
                                }))
                            }
                        }
                    }
                    "find_callers" => {
                        let symbol = args.get("symbol").and_then(|v| v.as_str()).ok_or_else(|| Error::protocol(ErrorCode::InvalidParams, "Missing symbol"))?;
                        let index = self.index.read().await;
                        let mut callers = Vec::new();
                        for (caller, metas) in index.functions.iter() {
                            if metas.iter().any(|m| m.calls.iter().any(|c| c == symbol)) {
                                callers.push(caller.clone());
                            }
                        }
                        callers.sort();
                        Ok(json!({
                            "content": [MessageContent::Text { text: format!("Callers of {}: {:?}", symbol, callers) }]
                        }))
                    }
                    "structural_search" => {
                        let function_name = args.get("function_name").and_then(|v| v.as_str()).ok_or_else(|| Error::protocol(ErrorCode::InvalidParams, "Missing function_name"))?;
                        let index = self.index.read().await;
                        let metas = index.functions.get(function_name).ok_or_else(|| Error::protocol(ErrorCode::InvalidParams, "Function not found"))?;

                        let target_fps: std::collections::HashSet<&str> =
                            metas.iter().map(|m| m.fingerprint.as_str()).collect();
                        let mut matches: Vec<String> = index.functions.iter()
                            .filter(|(_, ms)| ms.iter().any(|m| target_fps.contains(m.fingerprint.as_str())))
                            .map(|(n, _)| n.clone())
                            .collect();
                        matches.sort();
                        Ok(json!({
                            "content": [MessageContent::Text { text: format!("Functions with same structure as {}: {:?}", function_name, matches) }]
                        }))
                    }
                    "run_symbol_tests" => {
                        let path = args.get("path").and_then(|v| v.as_str()).ok_or_else(|| Error::protocol(ErrorCode::InvalidParams, "Missing path"))?.to_string();
                        let test_data_dir = args.get("test_data_dir").and_then(|v| v.as_str()).map(|s| s.to_string());

                        // The core engine compiles and executes test processes;
                        // run it off the async executor.
                        let report = tokio::task::spawn_blocking(move || {
                            llmlang::testing::run_tests(&path, test_data_dir.as_deref(), &[])
                        })
                        .await
                        .map_err(|e| Error::protocol(ErrorCode::InternalError, e.to_string()))?
                        .map_err(|e| Error::protocol(ErrorCode::InternalError, e))?;

                        // Map each failure to the target symbol's AST fingerprint.
                        let mut failures = serde_json::Map::new();
                        for r in report.results.iter().filter(|r| !r.passed) {
                            failures.insert(r.fingerprint.clone(), json!({
                                "symbol": r.name,
                                "panic_message": r.panic_message,
                            }));
                        }
                        let payload = json!({
                            "file": report.file,
                            "total": report.total,
                            "passed": report.passed,
                            "failed": report.failed,
                            "results": report.results,
                            "failures": failures,
                        });
                        Ok(json!({
                            "content": [MessageContent::Text { text: serde_json::to_string(&payload).unwrap_or_default() }]
                        }))
                    }
                    "patch_symbol" => {
                        let function_name = args.get("function_name").and_then(|v| v.as_str()).ok_or_else(|| Error::protocol(ErrorCode::InvalidParams, "Missing function_name"))?;
                        let new_body_val = args.get("new_body_ast").ok_or_else(|| Error::protocol(ErrorCode::InvalidParams, "Missing new_body_ast"))?;
                        let path_arg = args.get("path").and_then(|v| v.as_str());

                        let new_body_ast: Expr = serde_json::from_value(new_body_val.clone())
                            .map_err(|e| Error::protocol(ErrorCode::InvalidParams, format!("Invalid AST: {}", e)))?;

                        // 1. Find file path (errors if the name is ambiguous and no path was given)
                        let file_path = {
                            let index = self.index.read().await;
                            let meta = index.resolve_function(function_name, path_arg)
                                .map_err(|e| Error::protocol(ErrorCode::InvalidParams, e))?;
                            meta.path.clone()
                        };

                        // 2. Parse and replace inside a bounded scope so `Parser` drops before await
                        let mut new_content = String::new();
                        {
                            let content = std::fs::read_to_string(&file_path).map_err(|e| Error::protocol(ErrorCode::InternalError, e.to_string()))?;
                            let mut parser = Parser::new(Lexer::new(&content), file_path.clone())
                                .map_err(|e| Error::protocol(ErrorCode::InternalError, format!("Parse init error: {}", e)))?;
                            let mut exprs = parser.parse_module()
                                .map_err(|e| Error::protocol(ErrorCode::InternalError, format!("Parse error: {}", e)))?;

                            // 3. Find and replace
                            let mut found = false;
                            for expr in exprs.iter_mut() {
                                if let Expr::Define(name, _params, body, _exported) = expr {
                                    if name == function_name {
                                        *body = Box::new(new_body_ast.clone());
                                        found = true;
                                        break;
                                    }
                                }
                            }

                            if !found {
                                return Err(Error::protocol(ErrorCode::InternalError, "Function definition not found in parsed source file"));
                            }

                            // 4. Serialize back
                            use llmlang::compiler::ast::display::PrettyExpr;
                            for expr in exprs {
                                new_content.push_str(&format!("{}\n\n", PrettyExpr::new(&expr, 0)));
                            }
                        }

                        std::fs::write(&file_path, format!("{}\n", new_content.trim_end()))
                            .map_err(|e| Error::protocol(ErrorCode::InternalError, e.to_string()))?;

                        // 5. Re-analyze to update index
                        let _ = self.analyze_path(&file_path).await;

                        Ok(json!({
                            "content": [MessageContent::Text { text: format!("Successfully patched function: {}", function_name) }]
                        }))
                    }
                    _ => Err(Error::protocol(ErrorCode::MethodNotFound, format!("Tool {} not found", name))),
                }
            }
            _ => Err(Error::protocol(ErrorCode::MethodNotFound, format!("Method {} not found", method))),
        }
    }

    async fn shutdown(&self) -> Result<(), Error> {
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (read_tx, read_rx) = mpsc::channel::<String>(100);
    let (write_tx, mut write_rx) = mpsc::channel::<String>(100);
    let init_id = Arc::new(RwLock::new(None));
    let init_id_stdin = init_id.clone();
    let init_id_stdout = init_id.clone();

    // Stdin reader with normalization shim
    tokio::spawn(async move {
        let stdin = stdin();
        let mut reader = BufReader::new(stdin).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            let normalized_line = if let Ok(mut val) = serde_json::from_str::<Value>(&line) {
                let mut changed = false;
                if let Some(method) = val.get("method").and_then(|v| v.as_str()) {
                    if method == "initialize" {
                        if let Some(id) = val.get("id") {
                            *init_id_stdin.write().await = Some(id.clone());
                        }
                        if let Some(params) = val.get_mut("params").and_then(|p| p.as_object_mut()) {
                            if let Some(client_info) = params.remove("clientInfo") {
                                params.insert("implementation".to_string(), client_info);
                                changed = true;
                            }
                        }
                    } else if method == "notifications/initialized" {
                        val["method"] = json!("initialized");
                        changed = true;
                    }
                }
                if changed {
                    serde_json::to_string(&val).unwrap_or(line)
                } else {
                    line
                }
            } else {
                line
            };
            let _ = read_tx.send(normalized_line).await;
        }
    });

    // Stdout writer with normalization shim
    tokio::spawn(async move {
        let mut stdout = stdout();
        while let Some(line) = write_rx.recv().await {
            let normalized_line = if let Ok(mut val) = serde_json::from_str::<Value>(&line) {
                let mut changed = false;
                if let Some(id) = val.get("id") {
                    let mut lock = init_id_stdout.write().await;
                    if Some(id) == lock.as_ref() {
                        // This is the initialize response
                        if let Some(result) = val.get_mut("result") {
                            let caps = result.take();
                            *result = json!({
                                "protocolVersion": "2024-11-05",
                                "capabilities": caps,
                                "serverInfo": {
                                    "name": "llm-mcp",
                                    "version": env!("CARGO_PKG_VERSION")
                                }
                            });
                            changed = true;
                        }
                        *lock = None;
                    }
                }
                if changed {
                    serde_json::to_string(&val).unwrap_or(line)
                } else {
                    line
                }
            } else {
                line
            };
            let _ = stdout.write_all(normalized_line.as_bytes()).await;
            let _ = stdout.write_all(b"\n").await;
            let _ = stdout.flush().await;
        }
    });

    let transport = StdioTransport::new(read_rx, write_tx);
    let handler = Arc::new(LLMLangMCPHandler::new());
    let server = Server::new(Arc::new(transport), handler);

    server.start().await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(src: &str, filename: &str) -> Vec<Expr> {
        let mut parser = Parser::new(Lexer::new(src), filename.to_string()).unwrap();
        parser.parse_module().unwrap()
    }

    fn index_source(index: &mut CodebaseIndex, path: &str, src: &str) -> usize {
        index.index_exprs(path, parse(src, path))
    }

    #[test]
    fn reindexing_a_file_does_not_duplicate_entries() {
        let mut index = CodebaseIndex::new();
        let src = "# Point x y\n: main x y\n    + $ x $ y";
        for _ in 0..3 {
            index_source(&mut index, "a.llm", src);
        }
        assert_eq!(index.functions.get("main").map(|m| m.len()), Some(1));
        assert_eq!(index.shapes.len(), 1);
    }

    #[test]
    fn reindexing_drops_renamed_and_deleted_symbols() {
        let mut index = CodebaseIndex::new();
        index_source(&mut index, "a.llm", ": old_name x\n    $ x\n: gone x\n    $ x");
        index_source(&mut index, "a.llm", ": new_name x\n    $ x");
        assert!(index.functions.get("old_name").is_none());
        assert!(index.functions.get("gone").is_none());
        assert!(index.functions.get("new_name").is_some());
    }

    #[test]
    fn purge_only_affects_the_reindexed_file() {
        let mut index = CodebaseIndex::new();
        index_source(&mut index, "a.llm", ": main\n    1");
        index_source(&mut index, "b.llm", ": main\n    2");
        index_source(&mut index, "a.llm", ": main\n    3");
        let paths: Vec<&str> = index.functions["main"].iter().map(|m| m.path.as_str()).collect();
        assert_eq!(paths.len(), 2);
        assert!(paths.contains(&"a.llm") && paths.contains(&"b.llm"));
    }

    #[test]
    fn resolve_function_disambiguates_by_path() {
        let mut index = CodebaseIndex::new();
        index_source(&mut index, "a.llm", ": main\n    1\n: only_here\n    1");
        index_source(&mut index, "b.llm", ": main\n    2");

        // Unique name resolves without a path.
        assert_eq!(index.resolve_function("only_here", None).unwrap().path, "a.llm");
        // Ambiguous name without a path is an error naming the candidates.
        let err = index.resolve_function("main", None).unwrap_err();
        assert!(err.contains("multiple files") && err.contains("a.llm") && err.contains("b.llm"), "{}", err);
        // A path picks the right definition; a wrong path is an error.
        assert_eq!(index.resolve_function("main", Some("b.llm")).unwrap().path, "b.llm");
        assert!(index.resolve_function("main", Some("c.llm")).is_err());
        assert!(index.resolve_function("missing", None).is_err());
    }
}
