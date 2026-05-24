# Potential Future Enhancements

## Decoupling Compilation from OS Thread Stack
If real-world usage produces Abstract Syntax Trees (ASTs) that are deep enough to exceed the OS thread stack limit (typically 8MB on modern systems), the recursive code generation and parser will currently crash with a stack overflow.

**Proposed Solution:**
Convert the parser and code generation phases from relying on OS call-stack recursion to a heap-allocated trampoline (tail-call elimination) or an iterative stack machine. This ensures the compiler can theoretically process ASTs of unlimited depth, constrained only by available system RAM rather than fixed OS thread boundaries.

## First-Class AST Manipulation API (Semantic Patching)
Currently, code modifications require text-based diffs or regex-based replacement, which are fragile when handled by LLMs.
**Proposed Solution:**
Provide a native compiler API that allows the LLM to emit structural AST patches (e.g., `AST.insertNode(Parent, Child)`). This eliminates syntax errors during large refactoring and makes architectural pivots mathematically sound and highly predictable.

## Design-by-Contract & Intent Verification
LLM hallucinations can sometimes violate intended business rules if they stray from the Product Owner's constraints.
**Proposed Solution:**
Introduce native pre- and post-condition contracts into the language syntax. The PO defines the invariant business logic, and the compiler statically or dynamically enforces these contracts. If an LLM generates non-compliant code, the build fails immediately, maintaining trust.

## Natural Language Metadata Nodes
Comments are currently ignored by the compiler, leaving a disconnect between the PO's plain English requirements and the compiled binary.
**Proposed Solution:**
Treat specific natural language directives as compiled metadata nodes (e.g., `NL "Calculate regional tax here"`). The compiler can extract these nodes to automatically generate living documentation, map them to executed code blocks, and track implementation progress.

## Deterministic, Explicit State Management
Hidden state mutations and race conditions are notoriously difficult for LLMs to debug, often leading to endless hallucination loops.
**Proposed Solution:**
Double down on linear typing (move/consume semantics) and strict immutability defaults. By forcing all state transitions to be explicit and deterministically verifiable, the LLM is guided into writing robust code that eliminates whole classes of "heisenbugs."

## Native Test-Driven Scenarios (BDD/TDD)
Writing ad-hoc test suites creates friction between the PO's scenario definitions and the LLM's implementation code.
**Proposed Solution:**
Bake behavior-driven scenario definitions directly into the standard library. The PO can define high-level scenarios ("Given X, When Y, Expect Z"), and the compiler treats them as failing constraints until the LLM satisfies them, creating a tight, verifiable feedback loop.

## Built-in Traceability & Telemetry
When a system behaves unexpectedly in production, bridging the gap back to the LLM's original architectural choices is heavily manual.
**Proposed Solution:**
Auto-instrument generated code to include deterministic telemetry. Executed traces should map directly back to the specific AST nodes and LLM design decisions that authored them, allowing for instant, transparent debugging and high-fidelity context for the PO.
