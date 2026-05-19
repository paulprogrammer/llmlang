# LLMLang - a programming language optimized for LLMs

We're vibecoding anyway, why not optimize for an LLM?

This entire repository has been largely vibecoded with
humans acting as the product owners, and the LLM acting
as the developer.

## Overview
`llmlang` is a Turing-complete, polymorphic language optimized for LLM token usage and speed. Features include a prefix-arity AST, SoA memory, linear ownership, and an LLVM backend. The toolchain provides a unified Clang driver and an MCP server for structural search, enabling high-efficiency autonomous software engineering for AI agents.

## Documentation
- **[LLM Specification](./LLM_SPEC.md):** High-density language rules designed specifically for AI consumption.
- **[Design Guide](./DESIGN.md):** Deep dive into the philosophy, memory layout (SoA), and linear ownership system.
- **[User Guide](./USER_GUIDE.md):** End-to-end build-to-binary pipeline and syntax quick reference.
- **[Build Guide](./BUILD_GUIDE.md):** Instructions for compiling the Rust toolchain and LLVM dependencies.
- **[Installation Guide](./INSTALL_GUIDE.md):** Deployment options for system, user, or project-local availability.
- **[MCP Server Guide](./MCP_GUIDE.md):** How to use the `llm-mcp` binary for structural codebase traversal.
- **[Diagnostics](./DIAGNOSTICS.md):** Mapping of token-efficient error and warning codes (`E00x`, `W00x`).
- **[Release Guide](./RELEASE_GUIDE.md):** Versioning strategy and automated release process.
- **[License](./LICENSE):** GPLv3 with the `llmlang` Runtime Exception (Free to use, unrestricted output).

## Versioning
`llmlang` uses SemVer. Current version: **v0.1.7**

## Getting Started
To compile and link a program in one command:
```bash
./llm-clang examples/hello.llm -o hello_bin
./hello_bin
```
