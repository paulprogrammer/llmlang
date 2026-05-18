# llmlang Project Instructions

This project is a compiler for `llmlang`, a programming language optimized for LLM creation and maintenance.

## Core Mandates
- **Language:** Rust
- **Backend:** LLVM (via `inkwell`)
- **Philosophy:** Performance, Token Efficiency, and Structural Predictability.
- **Design:** See [DESIGN.md](./DESIGN.md) for the full specification.

## Workflow
- **Parsing:** Use a hand-written recursive descent parser or `pest` for the prefix-arity AST.
- **IR Generation:** Map the SoA (Struct of Arrays) data layout directly to LLVM IR for SIMD optimization.
- **Testing:** Every new operation or language feature must have a corresponding test case in `tests/`.

## LLM Optimization
- Avoid verbose keywords.
- Use De Bruijn indices for scope.
- Enforce linear typing (move/consume).
