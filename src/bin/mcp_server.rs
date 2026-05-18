use async_trait::async_trait;
use mcp_sdk_rs::{
    error::{Error, ErrorCode},
    server::{Server, ServerHandler},
    transport::{stdio::StdioTransport},
    types::{ClientCapabilities, Implementation, ServerCapabilities, Tool, ToolSchema, MessageContent},
};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use std::collections::HashMap;
use walkdir::WalkDir;
use llmlang::lexer::Lexer;
use llmlang::parser::{Parser, Expr};
use sha2::{Sha256, Digest};
use tokio::io::{stdin, stdout, AsyncBufReadExt, AsyncWriteExt, BufReader};

struct CodebaseIndex {
    functions: HashMap<String, Expr>,
    shapes: HashMap<String, Vec<String>>,
    call_graph: HashMap<String, Vec<String>>,
    fingerprints: HashMap<String, Vec<String>>, // hash -> [function_names]
}

impl CodebaseIndex {
    fn new() -> Self {
        Self {
            functions: HashMap::new(),
            shapes: HashMap::new(),
            call_graph: HashMap::new(),
            fingerprints: HashMap::new(),
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
                let mut parser = Parser::new(Lexer::new(&content));
                let expressions = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    parser.parse_module()
                })) {
                    Ok(exprs) => exprs,
                    Err(_) => {
                        eprintln!("Failed to parse {}", entry.path().display());
                        continue;
                    }
                };

                for expr in expressions {
                    match expr {
                        Expr::Define(name, _, body, _) => {
                            let calls = body.get_calls();
                            let fp = body.structural_fingerprint();
                            let mut hasher = Sha256::new();
                            hasher.update(&fp);
                            let hash = hex::encode(hasher.finalize());

                            index.functions.insert(name.clone(), *body);
                            index.call_graph.insert(name.clone(), calls);
                            index.fingerprints.entry(hash).or_default().push(name.clone());
                            count += 1;
                        }
                        Expr::Shape(name, fields, _) => {
                            index.shapes.insert(name, fields);
                        }
                        _ => {}
                    }
                }
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
        Ok(caps)
    }

    async fn handle_method(&self, method: &str, params: Option<Value>) -> Result<Value, Error> {
        match method {
            "tools/list" => {
                let tools = vec![
                    Tool {
                        name: "analyze_codebase".to_string(),
                        description: "Indexes all .llm files in the given path".to_string(),
                        input_schema: Some(ToolSchema {
                            properties: Some(json!({
                                "path": { "type": "string", "description": "Local path to the codebase" }
                            })),
                            required: Some(vec!["path".to_string()]),
                        }),
                        annotations: None,
                    },
                    Tool {
                        name: "search_symbols".to_string(),
                        description: "Searches for functions or shapes by name".to_string(),
                        input_schema: Some(ToolSchema {
                            properties: Some(json!({
                                "query": { "type": "string" }
                            })),
                            required: Some(vec!["query".to_string()]),
                        }),
                        annotations: None,
                    },
                    Tool {
                        name: "find_callers".to_string(),
                        description: "Finds all functions that call the given symbol".to_string(),
                        input_schema: Some(ToolSchema {
                            properties: Some(json!({
                                "symbol": { "type": "string" }
                            })),
                            required: Some(vec!["symbol".to_string()]),
                        }),
                        annotations: None,
                    },
                    Tool {
                        name: "structural_search".to_string(),
                        description: "Finds functions with a similar AST structure".to_string(),
                        input_schema: Some(ToolSchema {
                            properties: Some(json!({
                                "function_name": { "type": "string" }
                            })),
                            required: Some(vec!["function_name".to_string()]),
                        }),
                        annotations: None,
                    },
                ];
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
                    "find_callers" => {
                        let symbol = args.get("symbol").and_then(|v| v.as_str()).ok_or_else(|| Error::protocol(ErrorCode::InvalidParams, "Missing symbol"))?;
                        let index = self.index.read().await;
                        let mut callers = Vec::new();
                        for (caller, callees) in index.call_graph.iter() {
                            if callees.contains(&symbol.to_string()) {
                                callers.push(caller.clone());
                            }
                        }
                        Ok(json!({
                            "content": [MessageContent::Text { text: format!("Callers of {}: {:?}", symbol, callers) }]
                        }))
                    }
                    "structural_search" => {
                        let function_name = args.get("function_name").and_then(|v| v.as_str()).ok_or_else(|| Error::protocol(ErrorCode::InvalidParams, "Missing function_name"))?;
                        let index = self.index.read().await;
                        let expr = index.functions.get(function_name).ok_or_else(|| Error::protocol(ErrorCode::InvalidParams, "Function not found"))?;
                        
                        let fp = expr.structural_fingerprint();
                        let mut hasher = Sha256::new();
                        hasher.update(&fp);
                        let hash = hex::encode(hasher.finalize());

                        let matches = index.fingerprints.get(&hash).cloned().unwrap_or_default();
                        Ok(json!({
                            "content": [MessageContent::Text { text: format!("Functions with same structure as {}: {:?}", function_name, matches) }]
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
                                    "version": "0.1.3"
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
