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

## Tools Provided

| Tool | Description |
| :--- | :--- |
| `analyze_codebase` | Recursively parses all `.llm` files in a directory and builds the index. |
| `search_symbols` | Returns all functions or shapes matching a name query. |
| `find_callers` | Returns a list of functions that call a specific symbol. |
| `structural_search` | Finds all functions in the project that share the same AST structure as the target function. |

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
