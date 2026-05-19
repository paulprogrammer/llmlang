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

## 4. Building Libraries

To create a library that other projects (in `llmlang` or other languages) can consume:

### Step 1: Use the `export` keyword
Mark your functions and shapes for export.

```llm
// math_lib.llm
export # Vec2 i64 i64
export : add_vec2 !v1 !v2 
  + get !v1 x 0 get !v2 x 0
```

### Step 2: Build the Library and Signatures
Run the compiler with the `--emit-sig` flag to produce the object file and the token-efficient signature file.

```bash
llmlang math_lib.llm -o math_lib.o --emit-sig
# Produces: math_lib.o and math_lib.llms
```

### Step 3: Consume in another project
The LLM only needs to read `math_lib.llms` to understand how to call your library, saving context tokens.

```llm
// main.llm
// (The LLM sees math_lib.llms and knows Vec2 and add_vec2 exist)
: main
  @ add_vec2 ...
```

### Standard Libraries
For common math functions (sin, cos, abs, etc.), see the [llmlang-math](https://github.com/paulprogrammer/llmlang-math) implementation. It serves as a reference for creating and importing portable modules.

## 5. Language Quick Reference

| Operation | Syntax | Description |
| :--- | :--- | :--- |
| **Export** | `X ...` | Mark a definition for external consumption. |
| **Apply** | `@<n> func args` | Call a function with `<n>` arguments (defaults to 1). |
| **Branch** | `? cond t f` | Conditional branch (phi-merge). |
| **Move** | `⮞ ^index` | Consume a variable (Linear Ownership). |
| **Borrow** | `⚓ ^index` | Read a variable without consuming. |
| **De Bruijn** | `^0`, `^1` | Reference variables by scope depth. |
| **Shape** | `# Name i64 ...` | Define a Struct of Arrays memory layout. |
| **New** | `N Name count` | Allocate a new SoA instance. |
| **Get** | `G instance f idx`| Load value from SoA column. |
| **Set** | `S instance f idx v`| Store value to SoA column. |
| **Let** | `L name val body` | Create a local binding. |
| **Import** | `I mod symbol` | Import external symbol. |
| **Compare** | `=`, `<`, `>` | Compare two values (returns 0 or 1). |
| **Bitwise** | `&`, `|`, `^` | Bitwise AND, OR, XOR. |
| **String** | `"text"` | String literal. |
| **Len** | `ℓ str` | Get string length. |
| **Concat** | `⧉ s1 s2` | Concatenate two strings. |
| **Sub** | `✂ s start len` | Extract substring. |
| **Loc** | `🔍 s pat` | Find index of pattern in string. |
| **Regex** | `≈ s regex` | Match string against regex. |
| **System** | `📥 h`, `📤 h s` | Read/Write to/from file handles. |
| **Stringify**| `🧵 i64` | Convert integer to string. |
| **Split** | `🪓 s d idx` | Extract segment by delimiter. |

## 4. Understanding Diagnostics

If the compiler outputs a code like `E005` or `W001`, refer to [DIAGNOSTICS.md](./DIAGNOSTICS.md) for the human-readable mapping. These codes are optimized to save LLM tokens.
