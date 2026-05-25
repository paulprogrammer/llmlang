# llmlang MCP Agent Workflow Guide

Welcome, Agent. This MCP server provides native tools to semantically navigate and refactor `llmlang` codebases. To maximize your efficiency, do not rely on standard line-by-line text editing. Use the following workflow:

## 1. Prime the Index
Always begin by invoking `analyze_codebase`.
```json
{
  "name": "analyze_codebase",
  "arguments": { "path": "src" }
}
```
This forces the MCP server to parse all `.llm` files, generate AST representations, and build the structural fingerprints required by the other tools.

## 2. Locate Your Target
Use `search_symbols` to find the exact name of the function or shape you need to modify.
```json
{
  "name": "search_symbols",
  "arguments": { "query": "calculate_tax" }
}
```

## 3. Extract the AST
Once you have the exact symbol name, extract its current AST using `get_definition`.
```json
{
  "name": "get_definition",
  "arguments": { "name": "calculate_tax" }
}
```
*Note: Do not try to read the file with standard text tools. The AST returned here is what you need to mutate.*

## 4. Execute a Semantic Patch
To modify the code, build a new JSON representation of the AST body. Pass the function name and the new JSON AST to `patch_symbol`.
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
The server will automatically map your AST directly into valid `llmlang` prefix syntax and rewrite the target file deterministically.

## Advanced Strategies
- **Refactoring:** Use `find_callers` to locate all dependencies of a symbol before modifying its signature.
- **Code Discovery:** Use `structural_search` to find similar functions based on their AST fingerprints, even if their names differ.
