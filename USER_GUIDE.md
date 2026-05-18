# llmlang User Guide

`llmlang` is a token-optimized programming language designed for LLMs. This guide explains how to use the compiler output to create executable binaries.

## 1. Generating LLVM IR

To compile an `llmlang` source file (e.g., `program.llm`) into LLVM Intermediate Representation (IR):

```bash
llmlang program.llm > output.ll
```

## 2. Building an Executable

Once you have the `.ll` file, you can use `clang` to compile it into a native binary.

### Native Binary (x86_64 / ARM)

```bash
clang output.ll -o my_program
./my_program
```

### WebAssembly (Wasm)

If you have the `wasm32` target installed for LLVM:

```bash
clang --target=wasm32 -nostdlib -Wl,--no-entry -Wl,--export-all output.ll -o output.wasm
```

## 3. Language Quick Reference

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
