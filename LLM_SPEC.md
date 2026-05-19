# llmlang: LLM-Only Specification (v1.0)
[TOKEN_OPTIMIZED: HIGH_DENSITY]

## 1. Syntax & Grammar
* **Form:** Prefix-arity AST.
* **Tokens:** Single-char ASCII and UTF-8 symbols.
* **Comments:** `//` (Line comment).
* **Operators:** 
  * `+`, `-`, `*`, `/` : Binary Math. 
  * `⮞` : Move (Consume). `⮞ ^idx`
  * `⚓` : Borrow (Read). `⚓ ^idx`
  * `~` : MutBorrow. `~ ^idx`
  * `=` , `<`, `>` : Binary Comparison.
  * `&`, `|`, `^` : Bitwise.
  * `:` : Define. `: name args body`
  * `#` : Shape (SoA). `# Name f1 f2 ...`
  * `?` : Branch. `? cond t f`
  * `!` : Expand (Template). `! name`
  * `N` : New (SoA). `N Shape count`
  * `G` : Get (SoA). `G inst field idx`
  * `S` : Set (SoA). `S inst field idx val`
  * `X` : Export. `X ...`
  * `L` : Let (Local). `L name val body`
  * `I` : Import. `I mod symbol` (Resolves arity/shape via `.llmi`).
  * `^` : De Bruijn. `^0`, `^1` ...
  * `ℓ` : Len. `ℓ string`
  * `⧉` : Cat. `⧉ s1 s2`
  * `✂` : Sub. `✂ s start len`
  * `🔍` : Loc. `🔍 s pat`
  * `≈` : Reg. `≈ s regex`
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
  * `🛡️` : Trap (Recover). `🛡️ try fallback` (Caught panics run fallback).
  * `.` : Sequence. `. expr1 expr2` (Returns expr2).
  * `"` : String Literal. `"text"`

## 2. Memory & Ownership (AFFINE_TYPING)
1. **Rule:** Bindings can be consumed at most ONCE. Unconsumed bindings are auto-dropped at end of scope.
2. **Move (`⮞`):** Transfer ownership.
3. **Borrow (`⚓`):** Read-only access.
4. **MutBorrow (`~`):** Mutable access.

## 3. Data Layout (SoA)
* **Principle:** Struct of Arrays for SIMD-readiness.
* **Metadata:** Index 0 = Count. Index 1..n = Column Pointers.

## 4. Examples (Dense)
- Add: `+ 10 20`
- Let: `L x 10 + ⚓ x 5`
- If: `? = ⚓ x 10 "yes" "no"`
- Factorial (Recursion): `: fact n ? ⚓ n * ⚓ n @ fact - ⮞ n 1 ⮞ n`
- JSON Roundtrip: `: trip L u N User 1 . S ⚓ u id 0 1 L j 📦 ⚓ u 📦2 ⮞ j "User"`
- Env Access: `: config 🌍 "API_KEY"`
- Sequence: `: seq . 📤 1 "Part 1\n" 📤 1 "Part 2\n"`
