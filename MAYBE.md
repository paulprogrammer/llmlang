# Potential Future Enhancements

## Decoupling Compilation from OS Thread Stack
If real-world usage produces Abstract Syntax Trees (ASTs) that are deep enough to exceed the OS thread stack limit (typically 8MB on modern systems), the recursive code generation and parser will currently crash with a stack overflow.

**Proposed Solution:**
Convert the parser and code generation phases from relying on OS call-stack recursion to a heap-allocated trampoline (tail-call elimination) or an iterative stack machine. This ensures the compiler can theoretically process ASTs of unlimited depth, constrained only by available system RAM rather than fixed OS thread boundaries.
