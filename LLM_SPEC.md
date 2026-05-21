# llmlang: LLM-Only Specification (v1.0)
[TOKEN_OPTIMIZED: HIGH_DENSITY]

## 1. Syntax & Grammar
* **Form:** Prefix-arity AST.
* **Tokens:** Single-char ASCII and short keywords.
* **Comments:** `//` (Line comment).
* **Operators:** 
  * `+`, `-`, `*`, `/` : Binary Math. 
  * `>` : Move (Consume). `> ^idx`
  * `$` : Borrow (Read). `$ ^idx`
  * `~` : MutBorrow. `~ ^idx`
  * `=` , `<`, `gt`, `lt` : Binary Comparison (`<` or `lt` for less-than, `gt` for greater-than).
  * `&`, `|`, `xor` : Bitwise operations.
  * `:` : Define. `: name args body`
  * `#` : Shape (SoA). `# Name f1 f2 ...`
  * `?` : Branch. `? cond t f`
  * `` ` `` : Expand (Template). `` ` name ``
  * `N` : New (SoA). `N Shape count`
  * `G` : Get (SoA). `G inst field idx`
  * `S` : Set (SoA). `S inst field idx val`
  * `X` : Export. `X ...`
  * `L` : Let (Local). `L name val body`
  * `I` : Import. `I mod symbol` (Resolves arity/shape via `.llmi`).
  * `^` : De Bruijn or Trap. `^0`, `^1` refer to De Bruijn variables, while a standalone `^` acts as a Trap: `^ try fallback`.
  * `sl` : Len (String/Collection length). `sl string`
  * `sc` : Cat (String concatenation). `sc s1 s2`
  * `ss` : Sub (Substring). `ss s start len`
  * `sf` : Loc (String location/find). `sf s pat`
  * `sr` : Reg (Regex match). `sr s regex`
  * `(` : Read. `( handle`
  * `)` : Write. `) handle string`
  * `str` : Stringify. `str int`
  * `sp` : Split. `sp str delim idx`
  * `jp` : Pack (Serialize). `jp inst` -> JSON string.
  * `ju` : Unpack (Deserialize). `ju json "Shape"` -> inst.
  * `map` : Map (Iterator). `map inst "field" func` -> new inst.
  * `flt` : Filter (Iterator). `flt inst func` -> new inst.
  * `%` : Money. `%+`, `%-`, `%*`, `%/` (Fixed-point, 4 decimals).
  * `% str` : Money to String. `% str money` -> "$X.XXXX".
  * `!` : Panic. `! message` (Aborts with message).
  * `tn` : Time Now. Returns TAI64 label (`i64`).
  * `tns` : Time Nano. Returns TAI64 nanoseconds (`i64`).
  * `tz` : TimeZone. Returns local timezone name (string).
  * `tg` : Time Get. `tg T index` (0=Y, 1=M, 2=D, 3=H, 4=m, 5=S).
  * `ts` : Time Set. `ts Y M D H m S` -> TAI64 label.
  * `env` : Environment. `env key` (Returns string).
  * `http` : HTTP client. `http method url body` (Returns response payload).
  * `srv` : HTTP server. `srv op_code arg` (Manage HTTP socket connection).
  * `.` : Sequence. `. expr1 expr2` (Returns expr2).
  * `"` : String Literal. `"text"`

## 2. Memory & Ownership (AFFINE_TYPING)
1. **Rule:** Bindings can be consumed at most ONCE. Unconsumed bindings are auto-dropped at end of scope.
2. **Move (`>`):** Transfer ownership.
3. **Borrow (`$`):** Read-only access.
4. **MutBorrow (`~`):** Mutable access.

## 3. Data Layout (SoA)
* **Principle:** Struct of Arrays for SIMD-readiness.
* **Metadata:** Index 0 = Count. Index 1..n = Column Pointers.

## 4. Examples (Dense)
- Add: `+ 10 20`
- Let: `L x 10 + $ x 5`
- If: `? = $ x 10 "yes" "no"`
- Factorial (Recursion): `: fact n ? $ n * $ n @ fact - > n 1 > n`
- JSON Roundtrip: `: trip L u N User 1 . S $ u id 0 1 L j jp $ u ju > j "User"`
- Env Access: `: config env "API_KEY"`
- Sequence: `: seq . ) 1 "Part 1\n" ) 1 "Part 2\n"`
