# LLMLang - a programming language optimized for LLMs

We're vibecoding anyway, why not optimize for an LLM?

This entire repository has been largely vibecoded with
humans acting as the product owners, and the LLM acting
as the developer.

## Overview

`llmlang` is a Turing-complete, statically-typed, and compiled programming language designed from the ground up for LLM-driven generation and maintenance. Rather than forcing LLMs to output verbose syntax and navigate ambiguous language semantics, `llmlang` uses an extremely token-efficient prefix-arity AST and strict structural rules that AI agents can predict with near-perfect accuracy. 

It compiles directly to highly-optimized LLVM IR via its native Rust backend, achieving C-like performance while strictly enforcing deterministic execution and safety.

## Key Features

- **Memory Safety via Linear Typing:** Features a robust linear ownership system (move/consume mechanics) paired with an automated compile-time drop checker. Resources and memory lifetimes are strictly deterministic, eliminating use-after-free and double-free vulnerabilities without the overhead of a garbage collector.
- **Automatic Parallelism Hoisting:** Through advanced dataflow analysis at the AST level, the compiler automatically extracts implicit parallelism. Independent execution branches and operations are hoisted into parallel threads via LLVM, dramatically boosting execution speeds without requiring developers to write explicit multi-threading code.
- **Native Web and Networking Stack:** The standard library includes battle-tested, high-performance networking capabilities out of the box:
  - **HTTP/HTTPS/TLS Support:** Fully functional server and client architectures powered natively by `mbedtls`, `curl`, and `picohttpparser`.
  - **Native JWT Processing:** Built-in cryptographic signature verification and JWT claims parsing for modern stateless authentication.
  - **Native JSON Serialization:** Effortless, type-safe marshalling between language primitives and JSON representations.
- **Business-Ready Core:** Designed to run next-generation business applications with precision math (`Money` types), functional data pipelines (`Map`, `Filter`, `Fold`), and advanced SoA (Struct of Arrays) memory layouts for SIMD vectorization.
- **Agentic Ecosystem:** Ships with its own MCP (Model Context Protocol) server for high-speed structural codebase traversal, allowing AI agents to navigate and refactor `llmlang` codebases with surgical precision.

## Code Examples

`llmlang` uses a highly compressed prefix-arity syntax that eliminates the need for trailing semicolons, deep nesting, or complex precedence rules.

### Hello World & HTTP Web Server
Launch a fully functional web server natively in just a few tokens:

```llm
// HTTP Native Server Example
: main
    L server srv 0 "8080"
    . ) 1 "Server started on 8080\n"

    // Accept an incoming connection
    L req ( $ server
    L path srv 2 $ req
    
    // Log the request path
    L log sc "Received request for: " $ path
    . ) 1 $ log

    // Respond and drop the request strictly via linear ownership
    . ) > req "Hello from llmlang!"

    // Drop the server
    . srv 4 > server
    0
```

### Business Logic & Functional Pipelines
Precision math types (`Money`) and automatic Struct-of-Arrays (SoA) layout mapping:

```llm
// Define a strictly typed SoA record
# Product id score price

// Multiply price by 2
: double_price p
    %* $ p %2.00

: main
    // Allocate an SoA buffer for 3 products
    L p N Product 3
    . S $ p id 0 1
    . S $ p price 0 %10.00
    // ... initialize others ...

    // Apply functional map and consume the original memory linearly
    L doubled map > p "price" double_price

    0
```

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
`llmlang` uses SemVer. Current version: **v0.3.1**

## Getting Started
To compile and link a program in one command:
```bash
./llm-clang examples/hello.llm -o hello_bin
./hello_bin
```
