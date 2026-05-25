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

*   **Atomic Now (`tn`):** Returns the current TAI64 label as an `i64`.
*   **Get Component (`tg T i`):** Decomposes a label into human-readable parts (0=Y, 1=M, 2=D, 3=H, 4=m, 5=S).
*   **Set Label (`ts Y M D H m S`):** Composes a TAI64 label from calendar parts.

Example:
```llm
: main
    L now tn
    L year tg $ now 0
    ) 1 sc "Current Year: " str > year
```

## 4. Cross-Module Imports (`.llmi`)

`llmlang` supports modular programming via structural signature files with the `.llmi` (LLM Interface) extension.

### The `.llmi` Workflow
When you compile a module with an output path, `llmlang` automatically generates a `.llmi` file. This file contains the signatures of all exported symbols (`X`) and shape definitions.

1.  **Define Library (`math.llm`):**
    ```llm
    X : add x y + $ x $ y
    ```
2.  **Compile Library:**
    ```bash
    ./llm-clang -c math.llm -o math.o
    # Generates math.o and math.llmi
    ```
3.  **Import in Client (`main.llm`):**
    ```llm
    I math add
    : main @2 add 10 20
    ```
4.  **Link and Run:**
    ```bash
    ./llm-clang math.o main.llm -o main
    ./main
    ```

`llm-clang` handles linking multiple `.llm` and `.o` files automatically. 

### Standard Libraries
For common math functions (sin, cos, abs, etc.), see the [llmlang-math](https://github.com/paulprogrammer/llmlang-math) implementation. It serves as a reference for creating and importing portable modules.

## 5. Language Quick Reference

| Operation | Syntax | Description |
| :--- | :--- | :--- |
| **Export** | `X ...` | Mark a definition for external consumption. |
| **Apply** | `@<n> func args` | Call a function with `<n>` arguments (defaults to 1). |
| **Branch** | `? cond t f` | Conditional branch (phi-merge). |
| **Move** | `> ^index` | Consume a variable (Linear Ownership). |
| **Borrow** | `$ ^index` | Read a variable without consuming. |
| **De Bruijn** | `^0`, `^1` | Reference variables by scope depth. |
| **Shape** | `# Name i64 ...` | Define a Struct of Arrays memory layout. |
| **New** | `N Name count` | Allocate a new SoA instance. |
| **Get** | `G instance f idx`| Load value from SoA column. |
| **Set** | `S instance f idx v`| Store value to SoA column. |
| **Let** | `L name val body` | Create a local binding. |
| **Import** | `I mod symbol` | Import external symbol. |
| **Compare** | `=`, `lt`, `gt` | Compare two values (returns 0 or 1). `<` can also be used for `lt`. |
| **Bitwise** | `&`, `\|`, `xor` | Bitwise AND, OR, XOR. |
| **String** | `"text"` | String literal. |
| **Len** | `sl str` | Get string length. |
| **Concat** | `sc s1 s2` | Concatenate two strings. |
| **Sub** | `ss s start len` | Extract substring. |
| **Loc** | `sf s pat` | Find index of pattern in string. |
| **Regex** | `sr s regex` | Match string against regex. |
| **System** | `( h`, `) h s` | Read/Write to/from file handles. |
| **Stringify**| `str i64` | Convert integer to string. |
| **Split** | `sp s d idx` | Extract segment by delimiter. |
| **JSON** | `jp inst`, `ju json "Shape"` | Serialize to/Deserialize from JSON. |
| **Map** | `map inst "f" func` | Map function over SoA column. |
| **Filter** | `flt inst func` | Filter SoA instance by predicate. |
| **Money** | `%+`, `%-`, `%*`, `%/` | Fixed-point precision math. |
| **MoneyStr**| `%str money` | Format money value to string. |
| **Panic** | `! message` | Abort execution with error message (also maps to ``` ` ```). |
| **Trap**  | `^ try fall` | Catch panic and run fallback (when not a De Bruijn index). |
| **Time**  | `tn`, `tg`, `ts` | TAI64 and Calendar primitives. |
| **TimeNano**| `tns` | High-resolution monotonic time (nanoseconds). |
| **TimeZone**| `tz` | Get local timezone name. |
| **Env** | `env key` | Access system environment variables. |
| **HTTP Client**| `http method url body` | Make an HTTP request. |
| **HTTP Server**| `srv op arg` | HTTP server socket manager operations. |
| **Metadata** | `M tag val target` | Attach key-value metadata to a definition. |
| **OtelEmit** | `oe type a1 a2 a3` | Push telemetry payload to async MPSC queue. |
| **Sequence** | `. e1 e2` | Evaluate e1 then e2, returning e2. |

## 6. Business Logic Example

```llm
# Invoice id amount

: process_tax inv
    %* $ inv %1.15  // 15% Tax

: main
    L i N Invoice 1
    . S $ i id 0 101
    . S $ i amount 0 %1000.00
    L taxed map > i "amount" process_tax
    L total G $ taxed amount 0
    L msg sc "Total with Tax: " %str > total
    . ) 1 > msg
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

## 9. OpenTelemetry Auto-Instrumentation

`llmlang` provides native OTEL span instrumentation with zero boilerplate. Tag a function definition with `M "otel" "span_name"` and the compiler auto-wraps the body with span entry/exit, timing, and trace context propagation.

### Auto-Instrumented Function

```llm
M "otel" "handle_request" : handle_request req
    + $ req 1
```

The compiler generates equivalent logic to:
1. Record start time (`llm_get_time_ns`)
2. Enter span (`llm_otel_enter_span`)
3. Execute function body
4. Record end time, emit span JSON, exit span

### Nested Spans

Nested auto-instrumented functions automatically propagate trace context via thread-local storage:

```llm
M "otel" "inner" : inner x
    + $ x 1

M "otel" "outer" : outer x
    L result @ inner $ x
    + > result 10
```

### Output Configuration

| Environment Variable | Behavior |
| :--- | :--- |
| `OTEL_EXPORTER_OTLP_ENDPOINT` (unset) | Spans written to stdout as JSON lines. |
| `OTEL_EXPORTER_OTLP_ENDPOINT=http://...` | Spans sent via HTTP POST to the endpoint. |

### Manual Emission (`oe`)

The `oe` operator pushes payloads directly to the async MPSC queue:

```llm
// oe type arg1 arg2 arg3
// type 2 = stdout, type 3 = HTTP POST
oe 2 > payload "" ""
```

The standard library `lib/otel.llm` provides the `OtelLog` shape and `emit_span` wrapper for structured manual telemetry.
