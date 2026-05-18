# LLM-Optimized Language Design Guide

## 1. Core Philosophy
* **Target Audience:** Large Language Models (LLMs).
* **Non-Goal:** Human readability.
* **Primary Goals:** 
  * Extreme token efficiency (minimizing context window usage).
  * High execution speed (close to metal, easy compilation to IR/WASM).
  * Predictability and safety during LLM generation.

## 2. Structural Mechanics
* **AST-Based Form:** The language is represented as a highly compressed Abstract Syntax Tree (AST), similar to S-expressions but optimized to remove unnecessary closing brackets (e.g., using prefix arity).
* **Implicit Parallelism:** Because the AST structure is explicit, pure nodes (no side effects, no mutable state) can be identified easily, allowing the compiler to automatically evaluate branches in parallel.
* **De Bruijn Indices:** Variables are referenced by their relative distance or index in the scope rather than by named identifiers. This saves tokens and eliminates the cognitive load of naming collisions for the LLM.

## 3. Memory & Ownership
* **No Garbage Collection:** The language operates without a garbage collector to ensure maximum execution speed and minimal runtime overhead.
* **Linear Typing:** Every variable or piece of data must be consumed exactly once. If data is no longer needed, it must be explicitly dropped. If it is needed multiple times, it must be explicitly copied/cloned.
* **Rust-style Borrowing:** 
  * Explicit markers for read-only borrowing and mutable borrowing.
  * No hidden global state; all required state transitions are explicitly passed through the AST nodes.

## 4. Module & Scope Structure (Manifest-Driven)
* **Deterministic Isolation:** Similar to `pnpm-lock.yaml`, each module has a strict metadata manifest that maps local, short aliases to exact cryptographic hashes or versions of dependencies.
* **Explicit & Compressed Wiring:** Inside the module's AST, the LLM uses the short alias defined in the manifest instead of long file paths. The runtime/compiler resolves this to the deterministic dependency.
* **Context Truncation:** When modifying a file, the LLM is only provided the function signatures of the dependencies declared in the local manifest. It never sees the full global dependency graph, keeping the context window incredibly lean.

## 5. Type System & Memory Layout (Data-Oriented)
* **Struct of Arrays (SoA):** The language natively enforces a columnar memory layout. Instead of defining Objects (Array of Structs), the LLM defines "Shapes" or "Tables". 
* **Cache Efficiency:** This enforces data-oriented design by default, yielding extreme CPU cache utilization and vectorization (SIMD) opportunities, fulfilling the execution speed goal.
* **LLM Predictability:** The LLM manages arrays of uniform primitives rather than calculating complex struct padding and alignment rules.

## 6. Primitive Operations Syntax
* **Composite Approach:** The language uses a hybrid syntax to balance extreme token efficiency with LLM pre-training alignment.
* **ASCII Symbols for Core Logic:** Base operators (math, borrow, move, apply, branch) are represented by single-character ASCII tokens (e.g., `+`, `>`, `&`, `@`, `?`). The apply operator supports an optional numeric suffix for explicit arity (e.g., `@2`) to handle bracketless prefix notation. This guarantees 1 char = 1 token for the most frequent operations.
* **Conditional Branching (`?`):** Implements `? cond true_branch false_branch`. Enforces linear stack consistency (both branches must leave the stack in the identical ownership state).
* **Short Mnemonics for Built-ins:** Standard library functions and common structural built-ins use 3-to-4 letter ASCII mnemonics (e.g., `len`, `map`, `sys`, `new`, `get`, `set`).

## 7. Vector Search & MCP Integration
* **Structural AST Embeddings:** The AST is inherently designed to be converted into structural embeddings. By vectorizing the sub-trees (the arrangement of operators, types, and data flow), an MCP service can index the "shape" of the logic.
* **Semantic Retrieval without Comments:** Because the code is structurally pure, the LLM can query the MCP service for "code that shapes data like X" or "pure functions transforming A to B" without relying on human-written comments. 
* **Zero-Shot Context Loading:** When the LLM encounters an unknown function hash, the tooling uses vector similarity on the structural embeddings to pull the most conceptually relevant documentation or implementation examples directly into the prompt.

## 8. Implementation Details
* **Implementation Language:** Rust (for memory safety, performance, and conceptual symmetry with the language's ownership model).
* **Compiler Backend:** LLVM via the `inkwell` crate.
* **Target Platforms:** Native Machine Code (x86_64, ARM) and WebAssembly (Wasm).
* **Computational Power:** Turing Complete (achieved via conditional branching and recursive function calls).
* **Parsing Strategy:** Hand-written Recursive Descent or PEG-based (using `pest`) to maintain absolute control over the dense, prefix-arity AST.

## 9. Library & Package Management
* **C ABI Default:** All exported functions use the standard C Calling Convention to ensure zero-cost interoperability with C, C++, Go, and Rust.
* **Signature Files (`.llms`):** The compiler generates high-density "header" files containing only `# Shape` and `: Function` signatures (no bodies). Consuming LLMs only need to read these files, saving thousands of tokens per dependency.
* **Content-Addressable Linking:** Packages are referenced in a local manifest by cryptographic hashes. The compiler automatically resolves these to the corresponding static (`.a`) or dynamic (`.so`) libraries.
* **Zero-Conf Interop:** Because `llmlang` produces standard object files, other languages can link to it using their native toolchains (e.g., `extern "C"` in Rust or `extern` in C++).
