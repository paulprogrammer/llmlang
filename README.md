# LLMLang - a programming language optimized for LLMs

We're vibecoding anyway, why not optimize for an LLM?

This entire repository has been largely vibecoded with
humans acting as the product owners, and the LLM acting
as the developer.

## Overview

`llmlang` is a Turing-complete, statically-typed, and compiled programming language designed from the ground up for LLM-driven generation and maintenance. Rather than forcing LLMs to output verbose syntax and navigate ambiguous language semantics, `llmlang` uses an extremely token-efficient prefix-arity AST and strict structural rules that AI agents can predict with near-perfect accuracy. 

It compiles directly to highly-optimized LLVM IR via its native Rust backend, achieving C-like performance while strictly enforcing deterministic execution and safety.

## Key Features

- **Memory Safety via Linear Typing:** Deterministic resource management using a linear ownership system (strict move/consume semantics) and compile-time drop checking. This guarantees zero use-after-free or double-free vulnerabilities with zero garbage collection overhead.
- **Transparent SIMD Auto-vectorization & OpenCL JIT GPU Dispatch:** High-performance hardware acceleration without boilerplate:
  - **Host-Target Optimization:** The compiler auto-detects host CPU architecture features (AVX/SSE/NEON) to generate highly optimized native vector operations.
  - **64-Byte Memory Alignment:** Built-in memory allocators enforce 64-byte alignment, satisfying hardware register alignment constraints for crash-free vector loops.
  - **Dynamic OpenCL JIT Compilation:** Functional `map` operations on Struct-of-Arrays (SoA) layouts are translated into OpenCL C kernels and JIT-compiled for GPU execution at runtime.
  - **Graceful Fallbacks:** If OpenCL runtime environments are absent, execution falls back automatically to vectorized CPU loops without crashing.
- **Pluggable Native Database Stack with Kubernetes Service Bindings:** Fully decoupled, self-registering driver architecture:
  - **Decoupled Drivers:** Connectors for SQLite, Redis, and MongoDB are self-registering constructor plugins. Change database drivers seamlessly by linking at compile time.
  - **Kubernetes Integration:** Out-of-the-box Kubernetes Service Bindings resolution. The runtime reads credentials automatically from projected binding directories (`SERVICE_BINDING_ROOT` or `/bindings/`), generating connection parameters transparently.
  - **Resource Lifetime Tracking:** Database connections are tracked natively as linear types, guaranteeing that database handles are safely closed and resources are released upon scope termination.
- **Implicit Parallelism Hoisting:** Statically analyzes the AST dataflow at compile time to automatically extract and hoist independent execution branches into parallel LLVM threads. This accelerates compute-intensive modules without manual multi-threading configurations.
- **Native Web and Networking Stack:** High-efficiency HTTP client and server architectures built directly into the runtime:
  - **Secure Communication:** Native HTTPS and TLS support powered by `mbedtls`, `curl`, and `picohttpparser`.
  - **Authentication & Serialization:** Built-in cryptographic JWT validation and high-speed JSON marshalling for stateless microservice APIs.
- **Business-First Primitives:** Primitives designed for high-density, production-grade applications, featuring precision `Money` math operations and functional data pipelines (`Map`, `Filter`, `Fold`).
- **AI-Agentic Ecosystem:** Includes a native Model Context Protocol (MCP) server that empowers LLMs to structurally traverse, analyze, and safely refactor `llmlang` codebases at speed.

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
`llmlang` uses SemVer. Current version: **v0.4.0**

## Getting Started
To compile and link a program in one command:
```bash
./llm-clang examples/hello.llm -o hello_bin
./hello_bin
```
