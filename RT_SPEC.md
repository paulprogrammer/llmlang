# llmlang: HTTP Router Specification (rt)
[TOKEN_OPTIMIZED: HIGH_DENSITY]

## 1. Syntax & Grammar
* **Form:** `rt port num_routes [method path handler]... fallback`
* **Arity:** Variadic, determined by `num_routes` (N). Total arguments = 3 + 3 * N.
* **Arguments:**
  * `port` : Port expression (e.g. `"8080"` or a variable).
  * `num_routes` : Compile-time constant integer representing the route count.
  * `method` : Route HTTP method string literal (e.g. `"GET"`).
  * `path` : Route path string literal (e.g. `"/api"`).
  * `handler` : Arity 1 function (consumes Request object, returns response).
  * `fallback` : Arity 1 function (consumes Request object, called if no route matches).

## 2. AST Expansion (Option B)
The parser expands `rt` at parse-time into standard `llmlang` expressions. No new LLVM codegen or C runtime changes.

### 2.1 Generated Loop Function (`_rt_loop_X`)
* **Signature:** `: _rt_loop_X server`
* **Accept Loop:**
  * `L req ( $ server` : Blocks and accepts incoming HTTP request.
  * `L method srv 1 $ req` : Extracts request method string.
  * `L path srv 2 $ req` : Extracts request path string.
* **Dispatch Tree:**
  * Uses nested `If` (`?`) and Bitwise And (`&`) operators.
  * String comparison uses anchored Regex matches (`sr`) to compare contents, preventing pointer equality issues.
  * For each route `i`:
    `? & sr $ method "^METHOD_i$" sr $ path "^PATH_i$" true_branch false_branch`
  * **True Branch:** `. @1 handler_i > req @1 _rt_loop_X > server` (Executes handler, recurses).
  * **False Branch:** The next route check, or fallback `. @1 fallback > req @1 _rt_loop_X > server`.

### 2.2 Entry Point
The in-place `rt` token expands to:
* `L s srv 0 port @1 _rt_loop_X > s` : Binds to port and executes loop, consuming the server handle.

## 3. Ownership & Memory
* **Request consumption:** Linear typing requires handlers to consume the request (`> req`).
* **Server consumption:** Tail recursion consumes the server handle (`> server`).
* **Auto-Drop:** Strings extracted via `srv 1` and `srv 2` (`method` and `path`) are automatically dropped by the compiler when scope ends.

## 4. Code Example (Dense)
```llm
// Route Handlers
: h1 req
    ) > req "Home Page"

: h2 req
    ) > req "{\"status\":\"ok\"}"

: h404 req
    ) > req "404 Not Found"

// Main entry
: main
    rt "8082" 2 "GET" "/home" h1 "POST" "/submit" h2 h404
    0
```
