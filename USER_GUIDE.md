# llmlang User Guide

`llmlang` is a token-optimized programming language designed for LLMs. This guide explains how to use the compiler output to create executable binaries.

## 1. Unified Clang Workflow

`llmlang` provides a wrapper script `llm-clang` that integrates directly with the Clang driver. This allows you to compile and link `.llm` files as if they were `.c` files.

### One-Command Build

```bash
./llm-clang my_program.llm -o my_program
./my_program
```

### End-to-End Example

```bash
# 1. Create source
echo ": main + 40 2" > test.llm

# 2. Build to binary
./llm-clang test.llm -o test_bin

# 3. Run
./test_bin
echo $?
# Output: 42
```

## 2. Advanced: Multi-Stage Build
...

## 3. Linking with External Libraries (C Interop)

`llmlang` can easily interface with C libraries. Since it outputs standard LLVM IR, you can link it with object files compiled from C.

### Calling C from llmlang
1.  **Declare the C function** (Future feature: currently requires manual IR editing or a stub).
2.  **Compile and Link**:
    ```bash
    clang my_c_code.c hello.ll -o combined_app
    ```

## 4. Language Quick Reference

| Operation | Syntax | Description |
| :--- | :--- | :--- |
| **Apply** | `@ func arg` | Call a function (recursive calls allowed). |
| **Branch** | `? cond t f` | Conditional branch (phi-merge). |
| **Move** | `> ^index` | Consume a variable (Linear Ownership). |
| **Borrow** | `& ^index` | Read a variable without consuming. |
| **De Bruijn** | `^0`, `^1` | Reference variables by scope depth. |
| **Shape** | `# Name i64 ...` | Define a Struct of Arrays memory layout. |
| **New** | `new Name count` | Allocate a new SoA instance. |

## 4. Understanding Diagnostics

If the compiler outputs a code like `E005` or `W001`, refer to [DIAGNOSTICS.md](./DIAGNOSTICS.md) for the human-readable mapping. These codes are optimized to save LLM tokens.
