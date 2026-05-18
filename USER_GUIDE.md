# llmlang User Guide

`llmlang` is a token-optimized programming language designed for LLMs. This guide explains how to use the compiler output to create executable binaries.

## 1. End-to-End Build Pipeline

Follow these steps to create, compile, and run your first `llmlang` program.

### Step A: Write the source
Create a file named `hello.llm`. To create an executable, you must define a `main` function.

```llm
// hello.llm
// A simple program that returns 42
: main
  + 40 2
```

### Step B: Compile to LLVM IR
Use the `llmlang` compiler to generate the intermediate representation.

```bash
llmlang hello.llm > hello.ll
```

### Step C: Link to Native Binary
Use `clang` to compile the IR into a machine-code executable.

```bash
clang hello.ll -o hello
```

### Step D: Run and Verify
Execute the binary. Since the program returns a value to the OS, check the exit code.

```bash
./hello
echo $?
# Output: 42
```

## 2. Targeting Different Platforms
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
