# llmlang: LLM-Only Specification (v1.0)
[TOKEN_OPTIMIZED: HIGH_DENSITY]

## 1. Syntax & Grammar
* **Form:** Prefix-arity AST.
* **Tokens:** Single-char ASCII preferred.
* **Operators:** 
  * `+`, `-`, `*`, `/` : Binary arithmetic.
  * `@` : Application. `@ func arg1 arg2`
  * `:` : Define. `: name arg1... body`
  * `#` : Shape (SoA). `# Name type1 type2...`
  * `>` : Move (Consume). `> ^index`
  * `&` : Borrow (Read). `& ^index`
  * `^n`: De Bruijn Index. `^0` = nearest scope.

## 2. Memory & Ownership (LINEAR_TYPING)
1. **Rule:** Every binding MUST be consumed exactly ONCE.
2. **Move (`>`):** Transfers ownership. Target becomes `E004` (unavailable).
3. **Borrow (`&`):** Concurrent read. Does not consume.
4. **Leak (`W001`):** Binding defined but never moved.

## 3. Data Layout (SOA_ENFORCED)
* **Keyword:** `new`, `get`, `set`.
* **Allocation:** `new ShapeName count_expr`. Returns pointer-struct.
* **Access:** 
  * `get pts x 0` -> Load row 0 of column 'x'.
  * `set pts x 0 val` -> Store val to row 0 of column 'x'.
* **Efficiency:** Columnar contiguous memory. SIMD-ready.

## 4. Diagnostic Codes
* **E003:** OOB Index.
* **E005:** Double Move (Invalid).
* **E006:** Unknown Shape.
* **W001:** Linear Leak.
Ref: DIAGNOSTICS.md

## 5. Examples (Dense)
- Add 1 to arg: `: add1 x + > ^0 1`
- SoA Set: `: init pts set > ^0 x 0 42`
- Move violation: `: fail x + > ^0 > ^0` -> `E005`
