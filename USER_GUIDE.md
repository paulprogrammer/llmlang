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

## 2. Compiler Configuration

`llmlang` supports external configuration via command-line flags or a JSON configuration file.

### CLI Options

| Flag | Description | Default |
| :--- | :--- | :--- |
| `-o <path>` | Set output binary/object path. | `a.out` |
| `-S, --emit-ir` | Emit LLVM IR instead of binary. | `false` |
| `-c, --config <file>` | Load a JSON config file. | `None` |
| `--parallel <n>` | Complexity threshold for auto-parallelism. | `50` |
| `--threads <n>` | Number of worker threads in the pool. | `8` |
| `--queue <n>` | Work-stealing queue size. | `64` |

### Configuration File (`llmlang.json`)

You can also use a JSON file for configuration. Flags provided on the CLI will override values in the file.

```json
{
  "parallel_threshold": 100,
  "max_threads": 4,
  "queue_size": 32
}
```

## 3. Temporal Logic (libtai Baseline)

`llmlang` uses a high-precision temporal model inspired by D.J. Bernstein's `libtai`. It distinguishes between **TAI64 labels** (atomic time) and **Calendar Time**.

*   **Atomic Now (`🕒`):** Returns the current TAI64 label as an `i64`.
*   **Get Component (`📅 T i`):** Decomposes a label into human-readable parts (0=Y, 1=M, 2=D, 3=H, 4=m, 5=S).
*   **Set Label (`📆 Y M D H m S`):** Composes a TAI64 label from calendar parts.

Example:
```llm
: main
    L now 🕒
    L year 📅 ⚓ now 0
    📤 1 ⧉ "Current Year: " 🧵 ⮞ year
```

## 4. Linking with External Libraries (C Interop)

`llmlang` can easily interface with C libraries. Since it outputs standard LLVM IR, you can link it with object files compiled from C.

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
| **JSON** | `📦 inst`, `📦2 json "Shape"` | Serialize to/Deserialize from JSON. |
| **Map** | `⟴ inst "f" func` | Map function over SoA column. |
| **Filter** | `▽ inst func` | Filter SoA instance by predicate. |
| **Money** | `💰+`, `💰-`, `💰*`, `💰/` | Fixed-point precision math. |
| **MoneyStr**| `💰🧵 money` | Format money value to string. |
| **Panic** | `🚨 message` | Abort execution with error message. |
| **Trap**  | `🛡️ try fall` | Catch panic and run fallback. |
| **Time**  | `🕒`, `📅`, `📆` | TAI64 and Calendar primitives. |
| **TimeNano**| `🕒⌛` | High-resolution monotonic time (nanoseconds). |
| **TimeZone**| `🕒🌍` | Get local timezone name. |
| **Env** | `🌍 key` | Access system environment variables. |
| **Sequence** | `. e1 e2` | Evaluate e1 then e2, returning e2. |

## 6. Business Logic Example

```llm
# Invoice id i64 amount i64

: process_tax inv
    💰* ⚓ inv 💰1.15  // 15% Tax

: main
    L i N Invoice 1
    . S ⚓ i id 101
    . S ⚓ i amount 💰1000.00
    L taxed ⟴ ⮞ i "amount" process_tax
    L total ⚓ taxed amount 0
    L msg ⧉ "Total with Tax: " 💰🧵 ⮞ total
    . 📤 1 ⮞ msg
    0
```

## 7. Testing

`llmlang` includes a self-hosted test suite for behavioral verification.

### Running Tests

```bash
# Run all tests (Rust + llmlang)
cargo test && ./llm-test
```

The `./llm-test` script compiles and runs all test programs in `tests/lang/*.llm`. You can also run specific tests:

```bash
./llm-test tests/lang/math.llm
```

## 8. Understanding Diagnostics

If the compiler outputs a code like `E005` or `W001`, refer to [DIAGNOSTICS.md](./DIAGNOSTICS.md) for the human-readable mapping. These codes are optimized to save LLM tokens.
