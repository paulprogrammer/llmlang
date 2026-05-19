# llmlang: LLM-Only Specification (v1.0)
[TOKEN_OPTIMIZED: HIGH_DENSITY]

## 1. Syntax & Grammar
* **Form:** Prefix-arity AST.
* **Tokens:** Single-char ASCII and UTF-8 symbols.
* **Operators:** 
  * `+`, `-`, `*`, `/` : Binary arithmetic (Auto-parallel if heavy).
  * `=`, `<`, `>` : Comparison (Returns 0 or 1).
  * `&`, `|`, `^` : Bitwise AND, OR, XOR.
  * `@` : Application. `@<n> func arg1...` (Auto-parallel arguments).
  * `?` : Branching. `? cond true_expr false_expr`
  * `:` : Define. `: name arg1... body`
  * `X` : Export. `X ...`
  * `L` : Let binding. `L name val body`
  * `I` : Import. `I module_alias symbol_name`
  * `#` : Shape (SoA). `# Name type1 type2...`
  * `N` : New (Alloc). `N Shape count`
  * `G` : Get (Load). `G inst field idx`
  * `S` : Set (Store). `S inst field idx val`
  * `⮞` : Move (Consume). `⮞ ^index`
  * `⚓` : Borrow (Read). `⚓ ^index`
  * `^n`: De Bruijn Index. `^0` = nearest scope.
  * `ℓ` : Length (String). `ℓ str`
  * `⧉` : Concat (String). `⧉ left right`
  * `✂` : Substring. `✂ str start len`
  * `🔍` : Location. `🔍 str pattern`
  * `≈` : Regex Match. `≈ str regex`
  * `📥` : Read. `📥 handle`
  * `📤` : Write. `📤 handle string`
  * `🧵` : Stringify. `🧵 int`
  * `🪓` : Split. `🪓 str delim idx`
  * `🕒` : Time Now. Returns TAI64 label (`i64`).
  * `📅` : Time Get. `📅 T index` (0=Y, 1=M, 2=D, 3=H, 4=m, 5=S).
  * `📆` : Time Set. `📆 Y M D H m S` -> TAI64 label.
  * `🌍` : Environment. `🌍 key` (Returns string).
  * `"` : String Literal. `"text"`

## 2. Memory & Ownership (AFFINE_TYPING)
1. **Rule:** Bindings can be consumed at most ONCE. Unconsumed bindings are auto-dropped at end of scope.
2. **Move (`⮞`):** Transfers ownership. Target becomes unavailable (`E004`).
3. **Borrow (`⚓`):** Concurrent read. Does not consume.

## 3. Automatic Parallelism
* **Heuristic:** The compiler identifies **Pure** (no `S`, `📥`, `📤`, `🕒`, `🌍`) and **Complex** sub-expressions.
* **Execution:** Heavy sub-trees are automatically forked to a background thread pool and synchronized via a fork-join model.

## 4. Execution & Entry Point
* **Binary Target:** Requires a `: main` function.
* **Runtime:** Linked with `rt.c` (Thread Pool, String Lib, libtai-Temporal, Env).

## 5. Diagnostic Codes
* **E003:** OOB Index.
* **E005:** Double Move.
* **E009:** Branch stack state mismatch.
Ref: DIAGNOSTICS.md

## 6. Examples (Dense)
- Add 1 to arg: `: add1 x + ⮞ ^0 1`
- Factorial (Recursion): `: fact n ? ^0 * ⚓ ^0 @ fact - ⮞ ^0 1 ⮞ ^0`
- Local Binding: `: calc x L y + ⮞ ^0 1 * ⚓ y ⚓ y`
- Library Import: `I math sin : test x @ sin ^0` (Ref: [llmlang-math](https://github.com/paulprogrammer/llmlang-math))
- String Concat: `: greet n ⧉ "Hello, " ⮞ ^0`
- Regex Check: `: is_digit s ≈ ⮞ ^0 "^[0-9]+$"`
- Env Access: `: config 🌍 "API_KEY"`
