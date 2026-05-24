# LLMLang Development & Writing Observations

This document serves as a living registry of observations, language behaviors, design discoveries, and syntax pitfalls compiled during work on the `llmlang` compiler and runtime. Refer to this when writing or modifying `llmlang` compiler/runtime code or when authoring self-hosted programs.

---

## 1. Syntax & Parser Pitfalls

### The `S` vs `G` Operator Nesting Trap
* **Discovery**: When reading values out of a Struct-of-Arrays (SoA) result struct (like database queries or HTTP routing structures), you must use the `G` (Get) operator.
* **Problem**: Using the `S` (Set) operator (e.g., `L value S $ results field 0`) causes the recursive descent parser to interpret the subsequent logic in the file as part of the fourth operand of the Set operation. This nests the remainder of the file, breaking router loops and subsequent function declarations.
* **Solution**: Always retrieve fields using `G`:
  ```llm
  // Correct
  L first_name G $ results name 0

  // Incorrect (breaks parser scope)
  L first_name S $ results name 0
  ```

---

## 2. Compiler & Codegen Rules

### FFI Function Naming
* **Discovery**: FFI functions declared with `I` (Import) should not use the `llm_` prefix in user code, even if the underlying C/Rust implementation defines them with the prefix.
* **Behavior**: The compiler automatically maps imported names (e.g. `db connect` or `http serve`) to their internal counterparts (`llm_db_connect` and `llm_http_serve`). Specifying the prefix manually in user code will cause compiler or symbol linkage failures.

### Pointer-returning FFI Registration
* **Discovery**: If an FFI function returns a pointer to an allocated resource (e.g., connection handles, parsed JSON nodes, etc.), the compiler must be explicitly made aware of it to track the variable's scope.
* **Mechanism**:
  - Add the function name to the `returns_ptr_with_stack` list in `src/compiler/analysis/mod.rs`.
  - Add both the prefixed and non-prefixed names to `ffi_funcs` list in `src/compiler/codegen/mod.rs`.
* **Impact**: If this registration is missing, the compiler will not generate the correct lexical drop instruction (`llm_drop`), leading to memory leaks and untracked resource scopes.

---

## 3. Runtime & Memory Semantics

### String Literal Header Magic
* **Discovery**: Strings created dynamically in the runtime have a header magic of `0x4C4C4D52` so the garbage collector can trace and free them.
* **Static Strings**: Compiler-generated string literals are stored in static read-only memory and have a header magic of `0` to prevent the GC from attempting to free them.
* **Failsafe**: Any runtime parameter binder or validator (such as SQLite column binders) must accept both magic values when identifying a valid string cell:
  ```c
  return (header->type == RT_TYPE_STRING && (header->magic == 0x4C4C4D52 || header->magic == 0));
  ```

### Zero-Parameter Dummy Structures
* **Discovery**: FFI parameter passing expects an active SoA pointer. You cannot pass raw integer `0` or null pointers to indicate "no parameters."
* **Solution**: Define a dummy shape and pass its reference:
  ```llm
  X # DummyParams val
  ...
  L dummy N DummyParams 1
  . S $ dummy val 0 0
  . @3 exec $ conn "DELETE FROM users" $ dummy
  ```

---

## 4. Linker & Build System Behaviors

### Constructor Stripping (Dead Code Elimination)
* **Discovery**: When building self-registering driver patterns (using `__attribute__((constructor))` in GCC/Clang), the linker will completely discard driver object files if they are not explicitly referenced by symbols in the main program.
* **Solution**: Force the linker to preserve all symbols inside the runtime static library (`libllm_rt.a`) by wrapping it with archive preservation flags in [llm-clang](file:///home/paul/PROJ/llmlang/llm-clang):
  - **Linux (GNU Linker)**: `-Wl,--whole-archive -lllm_rt -Wl,--no-whole-archive`
  - **macOS (Clang/LD)**: `-Wl,-force_load,libllm_rt.a`
