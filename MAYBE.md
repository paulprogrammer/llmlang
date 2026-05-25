# Potential Future Enhancements

## Decoupling Compilation from OS Thread Stack
If real-world usage produces Abstract Syntax Trees (ASTs) that are deep enough to exceed the OS thread stack limit (typically 8MB on modern systems), the recursive code generation and parser will currently crash with a stack overflow.

**Proposed Solution:**
Convert the parser and code generation phases from relying on OS call-stack recursion to a heap-allocated trampoline (tail-call elimination) or an iterative stack machine. This ensures the compiler can theoretically process ASTs of unlimited depth, constrained only by available system RAM rather than fixed OS thread boundaries.

## First-Class AST Manipulation API (Semantic Patching)
Currently, code modifications require text-based diffs or regex-based replacement, which are fragile when handled by LLMs.
**Proposed Solution:**
Provide a native compiler API that allows the LLM to emit structural AST patches (e.g., `AST.insertNode(Parent, Child)`). This eliminates syntax errors during large refactoring and makes architectural pivots mathematically sound and highly predictable.

## Intent & Contract Metadata Nodes
Currently, comments are ignored by the compiler, leaving a disconnect between the PO's plain English requirements and the compiled binary. Furthermore, LLM hallucinations can sometimes violate intended business rules if they stray from these unenforced constraints.
**Proposed Solution:**
Introduce unified "Intent Nodes" into the AST that capture both the natural language objective and formal pre/post-condition contracts. These nodes are stripped during LLVM IR generation (costing zero runtime overhead) but serve as a living requirements document. The PO defines the intent and invariants, allowing an LLM to statically verify that its implementation matches both the human description and the mathematical contract. The compiler can extract these nodes to automatically generate living documentation and track implementation progress.

## Deterministic, Explicit State Management
Hidden state mutations and race conditions are notoriously difficult for LLMs to debug, often leading to endless hallucination loops.
**Proposed Solution:**
Double down on linear typing (move/consume semantics) and strict immutability defaults. By forcing all state transitions to be explicit and deterministically verifiable, the LLM is guided into writing robust code that eliminates whole classes of "heisenbugs."

## Native Test-Driven Scenario Nodes (BDD/TDD)
Writing ad-hoc test suites creates friction, as tests often rot or fall out of sync with the underlying codebase when stored in separate directories. 
**Proposed Solution:**
Treat behavior-driven scenario definitions as a specialized type of metadata node attached directly to the function's AST. If "Intent Nodes" define internal constraints, "Scenario Nodes" define external invariants (Given X, Expect Y). By colocating the tests physically within the AST, the MCP Server can JIT-evaluate these scenarios to establish a verifiable, instant feedback loop for the LLM. Like all metadata nodes, these are completely stripped out during LLVM compilation, ensuring zero runtime overhead.

## ~~Built-in Traceability & Telemetry~~ ✅ Implemented
**Status:** Shipped. See [DESIGN.md §8](./DESIGN.md) and [USER_GUIDE.md §9](./USER_GUIDE.md).

Auto-instrumentation via `M "otel" "span_name"` metadata marker. Compiler injects span entry/exit, timing, and trace context propagation. All telemetry serialized through an async MPSC queue. Runtime-configurable output (stdout or HTTP) via `OTEL_EXPORTER_OTLP_ENDPOINT`. Standard library `lib/otel.llm` provides `OtelLog` shape and `emit_span` for manual telemetry.
