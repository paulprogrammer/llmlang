# llmlang Language Fundamentals (LLM-Zero-Shot-Guide)

[OBJECTIVE: DENSE_CONCEPT_MAPPING]
[FORMAT: SEMANTIC_COMPRESSION]

## 1. Core Paradigm
`llmlang` is a **Prefix-Arity AST** language with **Linear Ownership** and **Struct-of-Arrays (SoA)** memory. It is optimized for token density and SIMD-readiness.

## 2. Syntax & Structure
*   **Arity:** Operators are prefix and have fixed or explicit arity.
*   **Sequencing (`.`):** `. expr1 expr2` executes 1 then returns 2.
*   **Bindings (`L`):** `L name value body` creates a scope.
*   **De Bruijn (`^`):** Variables can be referenced by name or by scope index (`^0` = closest).

## 3. The UTF-8 Cheat Sheet
| Logic | Token | Pattern |
| :--- | :--- | :--- |
| **Move** | `⮞` | `⮞ ^idx` - Transfers ownership (Consumes). |
| **Borrow**| `⚓` | `⚓ ^idx` - Read-only access (No consume). |
| **Branch**| `?` | `? cond true_expr false_expr` (Must have same stack state). |
| **Apply** | `@` | `@func arg1 ...` (Use `@<n>` for n-arity). |
| **Trap**  | `🛡️` | `🛡️ try_expr fallback_expr` (Recover from `🚨`). |
| **SoA New**| `N` | `N Shape count` (Returns pointer). |
| **SoA Set**| `S` | `S inst field idx val`. |
| **SoA Get**| `G` | `G inst field idx`. |
| **Money** | `💰` | `💰+`, `💰-`, `💰*`, `💰/` (4-decimal fixed-point). |
| **Time**  | `🕒` | `🕒` (Now), `🕒⌛` (Nano), `🕒🌍` (TimeZone). |

## 4. Memory Safety (Affine Logic)
1.  **Consume-Once:** A variable is either **Moved** (`⮞`) once or **Dropped** (Auto-Drop).
2.  **Borrowing:** Use `⚓` for multiple reads.
3.  **Invalid Access:** Accessing a moved variable triggers `E004/E005`.
4.  **Capture Safety:** You cannot move (`⮞`) a variable inside a parallel task or trap (`🛡️`) if it was captured from a parent scope (triggers `E016`).

## 5. Performance Strategy (SoA)
Always prefer columnar access. Instead of an array of objects, use a single instance of a large Shape:
*   `# Point x y`
*   `L p N Point 1000` -> Allocates 2 contiguous arrays of 1000 `i64`s.
*   `⟴ p "x" func` -> Maps `func` over all `x` values with cache-friendly stride.

## 6. Error Handling
*   **Fatal:** Use `🚨 "msg"` for unrecoverable errors.
*   **Recoverable:** Wrap risky code in `🛡️`.
*   **Result Pattern:** Return `0` (null) for soft failures (common in `📦2` Unpack).

## 7. Canonical Patterns
**Recursive Loop:**
```llm
: loop i count
    ? < ⚓ i ⚓ count
        . ... // Work
          @2 loop + ⚓ i 1 ⚓ count
        0
```

**Financial Calculation:**
```llm
: calc_interest bal rate
    💰* ⚓ bal ⚓ rate
```
