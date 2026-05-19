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
* **Implicit Parallelism:** Because the AST structure is explicit, pure nodes (no side effects) can be identified easily. The compiler automatically evaluate heavy pure branches in parallel using a managed thread pool.
* **De Bruijn Indices:** Variables are referenced by their relative distance or index in the scope rather than by named identifiers. This saves tokens and eliminates the cognitive load of naming collisions for the LLM.

## 3. Memory & Ownership
* **No Garbage Collection:** The language operates without a garbage collector to ensure maximum execution speed.
* **Affine Typing (Auto-Drop):** Every variable can be consumed at most once. If a variable is unconsumed when its scope ends, the compiler automatically injects a drop call. This ensures memory safety without the manual overhead of a `free` or `drop` operator.
* **Rust-style Borrowing:** 
  * Explicit markers for read-only borrowing (`⚓`) and mutable borrowing (`~`).
  * No hidden global state; all state transitions are explicit in the AST.
* **Strings as Objects:** String literals and dynamic string results are treated as movable objects. Concatenation and other operations allocate from the heap via a small runtime (`rt.c`).

## 4. Module & Scope Structure
* **Signature Files (`.llms`):** The compiler generates high-density "header" files containing only signatures (no bodies). Consuming LLMs only need to read these files, saving thousands of tokens.

## 5. Data Layout (Data-Oriented)
* **Struct of Arrays (SoA):** The language natively enforces a columnar memory layout. 
* **Cache Efficiency:** This yields extreme CPU cache utilization and vectorization (SIMD) opportunities.

## 6. Primitive Operations Syntax
* **UTF-8 Symbols for Core Logic:** Base operators use single-character tokens (e.g., `+`, `⮞`, `⚓`, `@`, `?`). 
* **String Operations:** Native string support using UTF-8 symbols for length (`ℓ`), concatenation (`⧉`), substring (`✂`), location (`🔍`), regex match (`≈`), and split (`🪓`).
* **System I/O:** Explicit handle-based primitives for reading (`📥`) and writing (`📤`).
* **Environment Access:** System-level configuration access via the `🌍` operator.
* **Temporal Logic:** High-precision TAI64 labels and calendar primitives (`🕒`, `📅`, `📆`) based on the `libtai` baseline.

## 7. Implementation Details
* **Implementation Language:** Rust.
* **Compiler Backend:** LLVM via the `inkwell` crate.
* **Runtime Support:** A managed C runtime (`src/rt.c`) provides:
  * A task-based **Thread Pool** for automatic parallelism.
  * Heap-allocated string operations.
  * POSIX regex support.
  * TAI64 temporal math and leap-second-agnostic calendar logic.
* **Computational Power:** Turing Complete.
