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
* **Implicit Parallelism:** Because the AST structure is explicit, pure nodes (no side effects) can be identified easily. The compiler automatically evaluates heavy pure branches in parallel using a managed thread pool.
* **De Bruijn Indices:** Variables are referenced by their relative distance or index in the scope rather than by named identifiers. This saves tokens and eliminates the cognitive load of naming collisions for the LLM.

## 3. Memory & Ownership
* **No Garbage Collection:** The language operates without a garbage collector to ensure maximum execution speed.
* **Affine Typing (Auto-Drop):** Every variable can be consumed at most once. If a variable is unconsumed when its scope ends, the compiler automatically injects a drop call. This ensures memory safety without the manual overhead of a `free` or `drop` operator.
* **Rust-style Borrowing:** 
  * Explicit markers for read-only borrowing (`$`) and mutable borrowing (`~`).
  * No global state; all state transitions are explicit in the AST.
* **Strings as Objects:** String literals and dynamic string results are treated as movable objects. Concatenation and other operations allocate from the heap via a small runtime (`rt.c`).

## 4. Module & Scope Structure
* **LLM Interface Files (`.llmi`):** The compiler generates high-density "header" files containing only signatures (no bodies). Consuming LLMs and the compiler use these files for cross-module discovery, saving thousands of tokens.

## 5. Data Layout (Data-Oriented)
* **Struct of Arrays (SoA):** The language natively enforces a columnar memory layout. 
* **Cache Efficiency:** This yields extreme CPU cache utilization and vectorization (SIMD) opportunities.

## 6. Primitive Operations Syntax
* **ASCII Symbols for Core Logic:** Base operators use single-character ASCII tokens (e.g., `+`, `>`, `$`, `@`, `?`, `.`). 
* **Sequence Operator (`.`):** Implements `. expr1 expr2`, allowing multiple statements to be executed in order within a single-expression body.
* **String Operations:** Native string support using ASCII keywords for length (`sl`), concatenation (`sc`), substring (`ss`), location (`sf`), regex match (`sr`), split (`sp`), and string constructor (`str`).
* **System I/O:** Explicit handle-based primitives for reading (`(`) and writing (`)`).
* **Business Primitives:** High-level support for JSON serialize (`jp`) / deserialize (`ju`), iterative field/array processing (`map`, `flt`), and precision fixed-point math (`%`) for financial applications.
* **Error Handling & Fault Tolerance:** Explicit panic mechanism (`!`) for non-recoverable states and a scoped trap operator (`^`) for catching panics in long-running processes.
* **Compiler Configuration:** Tunable thresholds for auto-parallelism and thread pool management via CLI flags and JSON configuration.
* **Environment Access:** System-level configuration access via the `env` keyword.
* **Temporal Logic:** High-precision TAI64 labels and calendar primitives (`tn` (now), `tns` (nanoseconds), `tg` (get), `ts` (set)) based on the `libtai` baseline, including local timezone resolution (`tz` (timezone)).

## 7. Implementation Details
* **Implementation Language:** Rust.
* **Compiler Backend:** LLVM via the `inkwell` crate.
* **Runtime Support:** A modular C runtime (`src/runtime/`) provides:
  * A task-based **Thread Pool** with work-stealing joins for automatic parallelism.
  * Fault-tolerant execution using a thread-local jump-buffer stack.
  * Heap-allocated string and JSON operations.
  * POSIX regex support.
  * TAI64 temporal math and leap-second-agnostic calendar logic.
  * **MPSC Emission Queue** for serialized async I/O (telemetry, fire-and-forget HTTP).
  * **Thread-local trace context** for transparent OTEL span propagation.
* **Computational Power:** Turing Complete.

## 8. Observability & Telemetry
* **Metadata Nodes (`M`):** The `M tag value target` syntax attaches key-value metadata to any expression. The compiler strips metadata during LLVM IR generation (zero runtime cost) unless the tag triggers a compiler pass (e.g., `M "otel" "span_name" : func`).
* **Auto-Instrumentation:** When a function definition is tagged with `M "otel" "name"`, the compiler automatically wraps the body with:
  1. `llm_otel_enter_span()` — pushes trace context onto a thread-local stack.
  2. `llm_get_time_ns()` — records start/end timestamps.
  3. `llm_otel_emit_span()` — serializes a JSON span and routes to the MPSC queue.
  4. `llm_otel_exit_span()` — pops trace context.
* **MPSC Queue (`oe` operator):** The `oe type arg1 arg2 arg3` operator pushes payloads to a persistent flusher thread. Task types: `1` = discarded HTTP, `2` = stdout, `3` = HTTP POST, `4` = file append. This guarantees zero interleaving across producers.
* **Discarded Value Detection:** When the compiler detects that an `http` call's return value is discarded (e.g., left branch of a `.` sequence), it auto-reroutes the call to the MPSC queue as a fire-and-forget task.
* **Runtime Configuration:** Set `OTEL_EXPORTER_OTLP_ENDPOINT` to route spans to an HTTP collector. Unset means stdout JSON lines.
* **Standard Library:** `lib/otel.llm` exports `OtelLog` shape and `emit_span` function for manual telemetry emission.

