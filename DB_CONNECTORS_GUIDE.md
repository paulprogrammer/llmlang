# LLMLang Database Connectors Reference Guide

This guide details the syntax, architectural design, memory lifecycle, and usage rules for database connectivity in `llmlang`. It is structured to help an LLM write, parse, and debug `llmlang` database operations.

---

## 1. Import Signatures

Every source file using database operations must import the `db` library and declare the required FFI signatures.

```llm
I db connect 2
I db connect_binding 2
I db query 4
I db exec 3
I db error 1
```

* **`connect 2`**: Establishes direct driver connection.
* **`connect_binding 2`**: Resolves connection credentials via Kubernetes Service Bindings.
* **`query 4`**: Performs parameterized SELECT queries and returns a Struct-of-Arrays (SoA).
* **`exec 3`**: Runs mutating SQL statements (INSERT, UPDATE, DELETE, CREATE).
* **`error 1`**: Retrieves the last error message from the connection handle.

---

## 2. Memory Lifecycle and Resource Ownership

1. **Linear Connection Resources**:
   - Connection handles returned by `connect` and `connect_binding` are tracked as pointer-returning FFI resources (internally assigned type `RT_TYPE_DB` / `4`).
   - The variable storing the connection (e.g., `L conn ...`) is a **linear resource**.
   - **No Explicit Close Needed**: The compiler generates an automatic drop (`llm_drop`) for the connection variable when it exits its lexical scope. This automatically invokes the driver's underlying close operation and frees associated structures.

2. **Garbage Collection (GC) Compatibility**:
   - All string outputs returned by queries or errors are registered with the runtime's reference-counted garbage collector.
   - String literals inside the compiler possess a magic header value of `0` to prevent GC from freeing static memory. The SQLite driver handles both GC-allocated and static compiler literal strings during parameter binding.

---

## 3. Shape Declarations

Before performing database operations, declare target shapes representing records and query parameters. Shapes map positionally to query parameters and output columns.

```llm
// Output schema representing database rows
X # User id name age

// Query parameters shape for positional bindings
X # QueryParams min_age

// Dummy parameter shape (required if an operation takes no parameters)
X # DummyParams val
```

---

## 4. Connection Rules

### A. Direct Connection (`connect`)
* **Syntax**: `L conn @2 db connect <driver: string> <connection_string: string>`
* **Supported Drivers**: `"sqlite"`, `"redis"`, `"mongodb"`
* **Example**:
  ```llm
  L conn @2 db connect "sqlite" "sqlite://prod.db"
  ```
  *(Note: The sqlite driver skips the `sqlite://` prefix to parse the filepath directly).*

### B. Kubernetes Service Bindings (`connect_binding`)
* **Syntax**: `L conn @2 db connect_binding <driver: string> <binding_name: string>`
* **Behavior**: Reads credentials projected in `/bindings/<binding_name>/` (configurable via `SERVICE_BINDING_ROOT` environment variable).
* **Example**:
  ```llm
  L conn @2 db connect_binding "sqlite" "db-primary"
  ```
* **Credential Resolution Rules**:
  - The runtime checks for file keys `url` or `connection-string`. If found, it uses them directly.
  - If not found, it parses `database`, `path`, `host`, `port`, `username` (or `user`), and `password` files to construct the driver's connection string dynamically.

---

## 5. Parameterized Queries and Mutation

### A. Executing Queries (`query`)
* **Syntax**: `L results @4 db query $ conn <sql: string> <shape_name: string> $ params_soa`
* **Rules**:
  1. The 3rd parameter must be the **exact string name of the target shape** (e.g., `"User"`). The compiler matches this shape name at compile time to inject its comma-separated fields.
  2. The 4th parameter must be a reference to an instance of the parameters shape (`$ params_soa`). Positional parameters in the SQL query (`?`) map directly to the values in the parameters shape.
* **Return Value**: Returns a Struct-of-Arrays (SoA) layout. Use the index-based retrieval operator `G` to fetch fields.
* **Example**:
  ```llm
  L qp N QueryParams 1
  . S $ qp min_age 0 30
  L results @4 db query $ conn "SELECT id, name, age FROM users WHERE age > ?" "User" $ qp
  ```

### B. Executing Mutations (`exec`)
* **Syntax**: `L affected @3 db exec $ conn <sql: string> $ params_soa`
* **Rules**:
  1. Executes write/command operations (INSERT, UPDATE, DELETE, CREATE).
  2. Always requires a parameters shape instance. If no parameters are needed, instantiate a `DummyParams` shape with a length of `1` and pass it.
* **Return Value**: An integer representing rows affected, or `-1` on error.
* **Example**:
  ```llm
  L dummy N DummyParams 1
  . S $ dummy val 0 0
  L affected @3 db exec $ conn "DELETE FROM users" $ dummy
  ```

### C. Error Checking (`error`)
* **Syntax**: `L err_msg @1 db error $ conn`
* **Behavior**: Obtains the last driver-specific error. Returns an owned, garbage-collected string (returns empty string `""` if no error occurred).
* **Example**:
  ```llm
  L err_msg @1 db error $ conn
  ```

---

## 6. Critical Syntax Pitfalls for LLMs

* **Use `G` for Retrieving, Never `S`**:
  When reading query fields, use the `G` (Get) operator. Using `S` (Set) causes the parser to consume the remainder of the file as part of the Set node operand, resulting in nested routing issues.
  - **Correct**: `L bob_name G $ results name 0`
  - **Incorrect**: `L bob_name S $ results name 0`
* **Dummy Parameters Mandatory**:
  You cannot pass `0` or null values for the parameter argument in FFI. You must always pass an initialized shape instance.
* **Positional Binding Only**:
  Query parameters are positionally mapped to the variables in the parameters shape. Ensure shape properties order matches the order of placeholders (`?`) in the SQL query.
* **FFI Signatures**:
  Do not prefix runtime library names with `llm_` inside the `llmlang` source code. Prefix resolution is handled automatically by the compiler. Write `db connect`, not `db llm_db_connect`.

---

## 7. Complete Reference Example

The following code demonstrates establishing an SQLite connection, inserting records using a parameter shape, performing a query, and reading results.

```llm
// Import database interface
I db connect 2
I db query 4
I db exec 3
I db error 1

// Define Shapes
X # User id name age
X # QueryParams min_age
X # DummyParams val

: main
    // 1. Establish connection (auto-dropped on scope exit)
    L conn @2 connect "sqlite" "sqlite://app.db"

    // 2. Initialize dummy params for parameterless DDL executions
    L dummy N DummyParams 1
    . S $ dummy val 0 0
    . @3 exec $ conn "CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY, name TEXT, age INTEGER)" $ dummy
    . @3 exec $ conn "DELETE FROM users" $ dummy

    // 3. Prepare parameters and insert record
    L new_user N User 1
    . S $ new_user id 0 42
    . S $ new_user name 0 "Alice"
    . S $ new_user age 0 28
    . @3 exec $ conn "INSERT INTO users (id, name, age) VALUES (?, ?, ?)" $ new_user

    // 4. Query record using parameter binding
    L qp N QueryParams 1
    . S $ qp min_age 0 25
    L results @4 query $ conn "SELECT id, name, age FROM users WHERE age > ?" "User" $ qp

    // 5. Read fields from the result SoA
    L row_count sl $ results
    L first_name G $ results name 0
    L first_age G $ results age 0

    0
```
