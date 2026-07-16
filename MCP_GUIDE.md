# llmlang MCP Server Guide

The `llm-mcp` binary provides a Model Context Protocol (MCP) server that enables LLMs to efficiently traverse and analyze an `llmlang` codebase.

## Capabilities

### 1. Structural Analysis
The server uses the same parser as the compiler to build a high-fidelity AST of your project. It can:
- List all functions and shapes.
- Build a full call graph (who calls whom).
- Map shapes to the functions that consume them.

### 2. Structural Vector Search
Instead of generic text embeddings, `llm-mcp` generates **Structural Fingerprints** of AST subtrees. 
- It hashes the "shape" of the logic (operators and control flow) while ignoring variable names and literal values.
- This allows an LLM to search for "code that does the same thing" even if the naming is completely different.

## Resources Provided

| URI | Description |
| :--- | :--- |
| `llm://spec` | Token-by-token grammar, operator specification, memory safety rules, and canonical patterns. |
| `llm://fundamentals` | Alias for `llm://spec` (backward compatibility). |
| `llm://agent-workflow` | MCP server capabilities, tools, and strategic workflows for codebase analysis. |

## Tools Provided

| Tool | Description |
| :--- | :--- |
| `analyze_codebase` | Recursively parses all `.llm` files in a directory and builds the index. |
| `search_symbols` | Returns all functions or shapes matching a name query. |
| `get_definition` | Returns the **realized AST** and file location of a function or shape. |
| `get_diagnostics` | Runs the compiler's frontend on a file and returns **E00x/W00x** diagnostic codes. |
| `find_callers` | Returns a list of functions that call a specific symbol. |
| `structural_search` | Finds all functions in the project that share the same AST structure as the target function. |
| `run_symbol_tests` | Discovers `M "test"` functions in a `.llm` file, executes each in an isolated sandboxed process, and returns structured JSON mapping failures to the target symbol's **AST fingerprint**. Accepts an optional `test_data_dir` injected as `TEST_DATA_DIR` (default `./tests/data`). |

## Running the Server

The server communicates over **stdio** and is designed to be used by an MCP client (like Claude Desktop or a custom IDE integration).

```bash
# Build the server
cargo build --bin llm-mcp --release

# Run the server (stdio)
./target/release/llm-mcp
```

## Integration with the LLM
When an LLM uses this server, it can quickly orient itself in a large codebase by:
1.  Running `analyze_codebase` once.
2.  Using `structural_search` to find patterns of SoA data access or recursive logic.
3.  Mapping dependencies via `find_callers` without reading every source file.

## Agent Workflow

When using this MCP server, follow this workflow for maximum efficiency:

### 1. Prime the Index
Always begin by invoking `analyze_codebase`.
```json
{
  "name": "analyze_codebase",
  "arguments": { "path": "src" }
}
```
This forces the server to parse all `.llm` files, generate AST representations, and build the structural fingerprints.

### 2. Locate Your Target
Use `search_symbols` to find the exact name of the function or shape you need to modify.
```json
{
  "name": "search_symbols",
  "arguments": { "query": "calculate_tax" }
}
```

### 3. Extract the AST
Once you have the exact symbol name, extract its current AST using `get_definition`.
```json
{
  "name": "get_definition",
  "arguments": { "name": "calculate_tax" }
}
```
*Note: Do not try to read the file with standard text tools. The AST returned here is what you need to mutate.*

### 4. Execute a Semantic Patch
Build a new JSON representation of the AST body and pass it to `patch_symbol`.
```json
{
  "name": "patch_symbol",
  "arguments": {
    "function_name": "calculate_tax",
    "new_body_ast": {
      "BinaryOp": [
        "Mul",
        { "Borrow": { "Identifier": "amount" } },
        { "Float": 1.2 }
      ]
    }
  }
}
```
The server will automatically map the AST into valid `llmlang` prefix syntax and rewrite the target file deterministically.

### Advanced Strategies
- **Refactoring:** Use `find_callers` to locate all dependencies of a symbol before modifying its signature.
- **Code Discovery:** Use `structural_search` to find similar functions based on their AST fingerprints, even if their names differ.
