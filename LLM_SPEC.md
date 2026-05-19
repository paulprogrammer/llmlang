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
  * `#` : Shape (SoA). `# Name field1 field2...`
  * `N` : New (Alloc). `N Shape count`
  * `G` : Get (Load). `G inst field idx`
  * `S` : Set (Store). `S inst field idx val`
  * `⮞` : Move (Consume). `⮞ name`
  * `⚓` : Borrow (Read). `⚓ name`
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
  * `📦` : Pack (Serialize). `📦 inst` -> JSON string.
  * `📦2`: Unpack (Deserialize). `📦2 json "Shape"` -> inst.
  * `⟴` : Map (Iterator). `⟴ inst "field" func` -> new inst.
  * `▽` : Filter (Iterator). `▽ inst func` -> new inst.
  * `💰` : Money. `💰+`, `💰-`, `💰*`, `💰/` (Fixed-point, 4 decimals).
  * `💰🧵`: Money to String. `💰🧵 money` -> "$X.XXXX".
  * `🚨` : Panic. `🚨 message` (Aborts with message).
  * `🕒` : Time Now. Returns TAI64 label (`i64`).
  * `🕒🌍`: TimeZone. Returns local timezone name (string).
  * `📅` : Time Get. `📅 T index` (0=Y, 1=M, 2=D, 3=H, 4=m, 5=S).
  * `📆` : Time Set. `📆 Y M D H m S` -> TAI64 label.
  * `🌍` : Environment. `🌍 key` (Returns string).
  * `.` : Sequence. `. expr1 expr2` (Returns expr2).
  * `"` : String Literal. `"text"`

## 2. Memory & Ownership (AFFINE_TYPING)
1. **Rule:** Bindings can be consumed at most ONCE. Unconsumed bindings are auto-dropped at end of scope.
2. **Move (`⮞`):** Transfers ownership. Target becomes unavailable (`E004`).
3. **Borrow (`⚓`):** Concurrent read. Does not consume.
4. **Auto-Drop:** SoA structures are recursively freed when they go out of scope.

## 3. Name Resolution
* **Automatic:** Identifiers (e.g. `u`, `json`) are automatically mapped to De Bruijn indices during parsing.
* **Mixed Mode:** Both named identifiers and explicit indices (`^0`) are supported.

## 4. Automatic Parallelism
* **Heuristic:** The compiler identifies **Pure** (no `S`, `📥`, `📤`, `🕒`, `🌍`) and **Complex** sub-expressions.
* **Execution:** Heavy sub-trees are automatically forked to a background thread pool and synchronized via a fork-join model.

## 5. Execution & Entry Point
* **Binary Target:** Requires a `: main` function.
* **Runtime:** Linked with modular C runtime (IO, Memory, Strings, Threads, Time, JSON).

## 6. Diagnostic Codes
* **E003:** OOB Index.
* **E005:** Double Move.
* **E006:** Unknown Shape.
* **E009:** Branch stack state mismatch.
Ref: DIAGNOSTICS.md

## 7. Examples (Dense)
- Add 1 to arg: `: add1 x + ⮞ x 1`
- Factorial (Recursion): `: fact n ? ⚓ n * ⚓ n @ fact - ⮞ n 1 ⮞ n`
- JSON Roundtrip: `: trip L u N User 1 . S ⚓ u id 0 1 L j 📦 ⚓ u 📦2 ⮞ j "User"`
- Env Access: `: config 🌍 "API_KEY"`
- Sequence: `: seq . 📤 1 "Part 1\n" 📤 1 "Part 2\n"`
