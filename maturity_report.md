# llmlang Maturity Report

**Date**: 2026-07-16 · **Version audited**: v0.5.0 (`main` @ `3ebb79d`)

A full-codebase scan for inefficiencies, redundancies, and antipatterns, with estimated impact and difficulty to resolve. Scope: ~5.5k lines of first-party Rust (compiler, CLI, MCP server, test harness), ~3.3k lines of first-party C runtime, shell tooling, and CI. Vendored code (`cJSON.c`, `picohttpparser.c`) excluded. Every finding was verified against the source at the time of writing; line numbers will drift.

**Difficulty scale**: **trivial** (< 30 min) · **small** (hours) · **medium** (~a day) · **large** (multi-day / architectural).

---

## Tier 1 — Correctness and safety bugs (will bite users)

### 1. AST pretty-printer emits source the parser cannot re-read (8 round-trip bugs)
`src/compiler/ast/display.rs`

The MCP `patch_symbol` tool rewrites whole source files through `PrettyExpr`, so any patched file containing these constructs is **silently corrupted**:

| Construct | Printed | Should be | Re-parses as |
|---|---|---|---|
| `DeBruijn(0)` | `D0` | `^0` | `Identifier("D0")` — hits every bound variable |
| `Unpack` | `ju "Shape" expr` | `ju expr "Shape"` | shape literal consumed as the expr |
| `Panic` | `` ` `` | `!` | `Expand` |
| `Expand` | `!name` | `` `name `` | `Panic` |
| `Eq` | `==` | `=` | two `Eq` tokens |
| `Gt` | `>` | `gt` | `Move` |
| export prefix | `*` | `X ` | `Mul` |
| expand param | `!name` (before) | `` name` `` (after) | plain param |

Root cause: the `!` vs `` ` `` mapping is inverted between the AST doc-comments and the actual lexer, and `display.rs` follows the wrong comments. This is likely why `patch_test.llm` is excluded from CI.

- **Impact**: high — data-destroying for the MCP patch workflow.
- **Difficulty**: trivial–small; each fix is a one-liner. Add a parse→print→parse round-trip property test and re-enable `patch_test.llm`.
- **Status: FIXED (2026-07-17, `maturity-work` branch)**. All 8 bugs fixed, plus 4 more found while writing the round-trip suite: `format_token` emitted `???` for `&`/`|`/`xor`; `Import` printed a trailing arity the parser rejects; whole floats printed as integers (`2.0` → `2` → `Integer`); string escaping missed `\` `\n` `\t` `\r`. 11 round-trip tests added in `display.rs` (including real files from `tests/lang/`), and the inverted `!`/`` ` `` doc-comments in `ast/mod.rs` corrected. MCP patch flow verified end-to-end in isolation. Notes: `patch_test.llm`'s exclusion from `llm-test` is legitimate (it is a fixture for `run_semantic_patch.py`, and its `main` takes params, so it isn't standalone-runnable); `run_semantic_patch.py` against the full `tests/lang/` dir still fails, but because of finding #6 — `patch_symbol("main")` matches another file's `main` and rewrites that file instead.

### 2. `llm_read` on raw fds: `FILE*` leak, silent data loss, null-deref
`src/runtime/io.c:26` — `fgets(..., fdopen((int)handle, "r"))` creates a fresh stdio stream per call: never `fclose`d (leak per read), its read-ahead buffer swallows bytes after the first line (subsequent reads lose data), and a NULL `fdopen` return is passed straight to `fgets` (crash).

- **Impact**: high — core `(` operator.
- **Difficulty**: small — cache the `FILE*` in a managed handle or use `read()` with a line buffer; add the NULL check.
- **Status: FIXED (2026-07-17, `maturity-work` branch)**. The raw-fd path now reads byte-at-a-time via `read()` (with EINTR retry): no `FILE*` exists to leak or NULL-deref, and nothing is consumed past the newline, so consecutive reads see consecutive lines and unread data stays on the fd. A bad fd now returns 0 instead of crashing. Regression test `tests/lang/stdin_multiline_test.llm` reads three piped lines plus EOF (run by `llm-test` with piped stdin; verified to fail with "line 2 mismatch" against the old code). The managed `LlmFile` path was already correct and is unchanged.

### 3. Non-atomic refcounts on objects shared across threads
`src/runtime/memory.c:8,34–35,95`, `common.h:35` — `ref_cnt` is a plain `unsigned short` mutated with `++`/`--`, while `llm_fork` hands the same managed pointers to pool threads. Racing dup/drop → lost counts → premature free → use-after-free. The 16-bit counter can also wrap.

- **Impact**: high — memory-unsafe under the language's own auto-parallelism.
- **Difficulty**: small — `_Atomic` ops, widen to 32-bit.
- **Status: FIXED (2026-07-17, `maturity-work` branch)**. `ref_cnt` is now `_Atomic unsigned int`: dup is `fetch_add` (relaxed), drop is `fetch_sub` (acq_rel) with only the releaser of the last reference destroying. The header was reordered to `{magic: u32, type: u32, ref_cnt: u32}` — three 4-byte fields, no padding — and `gen_string_constant` updated in lockstep (it materializes this header for string constants, previously as `{i32,i16,i16}`). New C-level regression test `tests/runtime/refcount_race_test.c` (8 threads × 200k balanced dup/drop, run by `llm-test`) fails against the old code with "object was freed while references remained" and passes now. Bonus fix: `llm-clang` now rebuilds runtime objects when `common.h` is newer, so header layout changes can no longer silently link stale objects (part of finding #36).

### 4. `handle > 1000` heuristic conflates file descriptors with heap pointers
`io.c:4,34`, `memory.c:31,92,102`, and scattered — an fd above 1000 (easy under load) is dereferenced as `LlmRtHeader*` at `handle - sizeof(header)`: an arbitrary memory read used as a type check.

- **Impact**: high — unsound; latent crash/security issue on busy servers.
- **Difficulty**: medium — wrap fds in managed handles or tag handle bits.

### 5. JSON root tracking: thread-local table with a silent 256 cap
`src/runtime/json.c:4–12` — past 256 live roots, registration is silently skipped, after which `get_node` misreads a wrapper cell as a raw `cJSON*` (crash/corruption). The table is `__thread`, so a handle dropped on another thread consults the wrong table.

- **Impact**: high for JSON-heavy or threaded workloads.
- **Difficulty**: medium — put a discriminator flag in `LlmRtHeader` instead of a side table.

### 6. MCP `patch_symbol` corrupts its own index and can crash the server
`src/bin/mcp_server.rs:100,447–448` — re-analysis appends to `fingerprints` without clearing (every patch duplicates entries; renamed/deleted symbols persist), and the trailing-newline write uses bare `.unwrap()` — with `panic = "abort"` in release, a racing file deletion kills the long-lived server for all clients.

- **Impact**: medium-high — wrong tool output + client-triggerable DoS.
- **Difficulty**: small.
- **Status: FIXED (2026-07-17, `maturity-work` branch)**. The index was restructured so it cannot go stale by design: `call_graph`/`fingerprints` side maps were folded into per-symbol metadata keyed name → one entry per defining file, and re-indexing a file purges its old entries before inserting. The unwrap-based trailing-newline append was replaced with a single `fs::write`. Also fixed the related wrong-file targeting found while validating #1: `patch_symbol` on a name defined in multiple files (e.g. `main`) silently rewrote whichever file last won the index — it now rejects the ambiguous call listing the candidates, and takes an optional `path` argument to disambiguate. 4 index unit tests added; `tests/run_semantic_patch.py` now asserts both the rejection and the disambiguated patch, and runs in CI on both platforms.

### 7. Compiler aborts instead of reporting errors; all semantic errors say line 1
`codegen/expr.rs` (many sites), `codegen/mod.rs:111`, `analysis/verify.rs:87` — `gen_expr` returns a bare value, so E003/E006/E007/E008/E010/E012/E013 are raised via `panic!`/`.expect()`; under `panic = "abort"` the compiler hard-aborts with no diagnostic formatting. Separately, every `CompileError` from verify/codegen hardcodes `line: 1`, so semantic errors cannot be located. Related: MCP serves `line: 0` placeholders as real symbol locations (`mcp_server.rs:97,104`).

- **Impact**: medium-high — robustness and the diagnostics UX the language advertises.
- **Difficulty**: large to thread `Result` through codegen; medium to carry real spans through the AST.

### 8. HTTP server: ignored `write()` returns, TLS `WANT_READ` treated as EOF, unbounded request allocation
`http_server.c:322,333,354–355` (partial writes silently truncate responses); `:224–231` with `tls.c:141–149` (transient `WANT_READ`/`EAGAIN` abandons valid HTTPS requests); `:266,284,309` (unchecked `realloc`/`malloc`, and **no cap on `Content-Length`** — a client can drive allocation arbitrarily high).

- **Impact**: medium-high for anyone using the HTTP server primitives (correctness + DoS vector).
- **Difficulty**: small each.

### 9. Floats silently truncated to integers
`sqlite_driver.c:100–101` (SQLite `FLOAT` cast to `long`), `json.c:163–165` (`json_get_float` is literally an alias of `json_get_int`).

- **Impact**: medium — silent data loss.
- **Difficulty**: small–medium, depending on whether the ABI can carry doubles.

### 10. `cms_unwrap` does not parse CMS
`crypto_cms.c:96–110` — fixed byte offsets (256-byte key, 16-byte IV) instead of ASN.1 traversal; misreads or overreads real CMS blobs. mbedtls return codes, `sscanf` results, and mallocs unchecked throughout `crypto*.c`.

- **Impact**: medium — security-sensitive code that only works on its own test vectors.
- **Difficulty**: large to parse properly; the unchecked-returns cleanup is trivial.

---

## Tier 2 — Performance inefficiencies

| # | Finding | Location | Impact | Difficulty |
|---|---|---|---|---|
| 11 | Whole-module verification runs **twice** (`verify_module`, then again per-function in `gen_function`), cloning shape/function maps per function | `main.rs:301`, `codegen/mod.rs:71–113`, `verify.rs:79–80` | ~2× verification cost, grows with module size | medium |
| 12 | **FIXED (2026-07-17)** — `get_functions().count()` as unique-ID generator per trap/parallel site → O(n²) codegen | `expr.rs:1089`, `parallel.rs:17` | moderate on trap-heavy code | trivial (monotonic counter) |
| 13 | **FIXED (2026-07-17)** — `get_module_name()`/`mangle_name()` re-parse the path and re-hash on **every identifier resolution** | `symbol.rs:13–20,50–56` | hot-path allocation churn | small (compute once) |
| 14 | Fixed-point type inference (≤100 iters) recomputes names and re-walks all bodies per iteration | `codegen/mod.rs:322–364` | moderate | small |
| 15 | Template bodies deep-cloned at every call site; every arg subtree cloned for a rare db-query rewrite | `expr.rs:296–322,378` | moderate | medium |
| 16 | `prune_dead_code` rescans all expressions per worklist item → O(defs²) | `analysis/mod.rs:355–376` | small today, grows | medium |
| 17 | `regcomp` on every `sr` call; no compiled-pattern cache | `strings.c:44–54` | large for regex-in-loop | medium |
| 18 | **FIXED — global init (2026-07-17); connection reuse still open** — Fresh `curl_easy_init`/cleanup per request (no connection/DNS/TLS-session reuse); **no `curl_global_init` anywhere** (lazy-init race across threads) | `http.c`, `http_server.c` | latency on repeated calls; init race is a correctness footnote | trivial (global init) / medium (reuse) |
| 19 | Crypto ops re-seed entropy and re-parse PEM keys on every call | `crypto.c:34–47,151–155` | large per-op overhead | medium |
| 20 | `llm_join` busy-polls with a 1 ms `timedwait` despite completion signaling | `threads.c:114–144` | wasted CPU while joining | small |
| 21 | Test harness recompiles + relinks from scratch per file per run (PID-keyed temp dir, deleted after) | `testing/mod.rs:358–377` | dominant cost of `llmlang test` | medium (fingerprint-keyed cache) |
| 22 | **FIXED (2026-07-17)** — CI: no cargo/registry caching, no concurrency groups; every push cold-builds inkwell + LTO on 4 runners | `ci.yml`, `release.yml` | CI minutes/cost | small (`rust-cache` + `concurrency:`) |

---

## Tier 3 — Redundancies

| # | Finding | Location | Impact | Difficulty |
|---|---|---|---|---|
| 23 | Three copies of the same ~30-line curl routine (incl. duplicated `ResponseBuffer`/`write_callback`); fixes like #18 need triple edits | `http.c` vs `http_server.c` | maintainability | small |
| 24 | Pointer-returning-function policy duplicated in **four places** as string matching (`ends_with("_get"/"_query"/…)`); a user function named `foo_query` is misclassified | `analysis/mod.rs:37–57`, `codegen/mod.rs:105–114`, `expr.rs:371–374`, `mod.rs:448–458` | correctness + drift | medium (centralize; ideally signature metadata, not names) |
| 25 | SoA allocation/copy codegen pasted 3× (`New`/`Map`/`Filter`, ~80 lines); `Get`/`Set` field resolution near-identical | `expr.rs:144–255,735–755,913–1005` | maintainability | medium |
| 26 | `infer_shape` duplicated verbatim | `codegen/shape.rs:13–29`, `verify.rs:14–30` | maintainability | small |
| 27 | Binary-discovery logic duplicated across languages (`llm-clang` finds `llmlang`; `find_llm_clang` finds `llm-clang`); three hand-rolled arg parsers with inconsistent missing-value handling (compile path silently ignores dangling `-o`; `--threads abc` silently falls back to 8) | `llm-clang:5–22`, `testing/mod.rs:135–159`, `main.rs` | maintainability, surprising CLI behavior | medium |
| 28 | CI steps copy-pasted ~5× between `ci.yml` jobs and `release.yml` (byte-identical LLVM install blocks; LLVM `22` pinned in several places) | workflows | maintainability | medium (composite action) |
| 29 | Thin pass-through wrappers (`db_connect`→`llm_db_connect`, etc.) doubling the runtime export surface | `db.c:211–230`, `http_server.c:413–427`, `file.c:24–26` | maintainability | trivial |
| 30 | Dead code: `Expr::returns_ptr()` never called; `CodeGen::warnings` never used; `CodeGen::stack_size` write-only | `analysis/mod.rs:71`, `codegen/mod.rs:60,72` | hygiene | trivial |

---

## Tier 4 — Antipatterns and hygiene

| # | Finding | Location | Impact | Difficulty |
|---|---|---|---|---|
| 31 | Inconsistent runtime error signaling: failures variously return `0`, `""`, `-1`, or panic; callers can't distinguish "empty result" from "error" (`http` returning `""` on both timeout and empty body caused a 30s CI mystery in July 2026) | pervasive in runtime | correctness/debuggability | medium (define a convention) |
| 32 | `setenv` from `llm_panic` is not thread-safe (panics on pool threads can race `getenv` elsewhere); better home is a field in the `__thread` trap frame | `io.c:72` | low-probability race | small |
| 33 | Silent truncation via fixed buffers: 4096-byte lines (`io.c`), 1024-byte binding files (`db.c:103`), 100 HTTP headers, 32 JSON/SoA fields, 256 OTEL context slots | scattered | silent data loss at limits | small each; documenting limits is the cheap half |
| 34 | Nine `RefCell`/`Cell` fields on `CodeGen` because `gen_expr` takes `&self` — runtime borrow-panic risk, borrow churn in hot loops | `codegen/mod.rs:59–69` | maintainability | large (bundle with #7) |
| 35 | Magic numbers: `0x4C4C4D52`, the `>1000` sentinel, socket subtype ints, money scale `10000`, element size `8` | scattered | maintainability; amplifies #4 | trivial (named constants) |
| 36 | **FIXED (2026-07-17)** — Shell scripts lack `set -euo pipefail`: `llm-clang` doesn't check the runtime C compile or `ar` (failed runtime build links stale objects silently); `llm-test`'s fixed port 8080 forbids concurrent runs (bind failure swallowed); `libllm_opencl.so` copied into caller's cwd on every link; hardcoded Homebrew paths | `llm-clang`, `llm-test` | silent partial builds, flakiness | small |
| 37 | Thread-pool has no shutdown path (`pool->shutdown` never set; `cond_signal` not broadcast); lazy `dlopen` in OpenCL dispatch is unsynchronized; OTEL span IDs are clock-derived and collision-prone | `threads.c`, `opencl_dispatch.c:11–22`, `http_server.c:451,458` | resource hygiene / telemetry quality | small–medium |
| 38 | `gen_string_constant` names globals by 64-bit `DefaultHasher` — a hash collision silently aliases two string constants | `codegen/mod.rs:176–181` | improbable but silent miscompile | small |
| 39 | `strtok_r` in `llm_split` treats the delimiter as a character **set**, not a literal — `sp s ", " i` behaves unexpectedly | `strings.c:67–86` | surprising semantics | small |

**Cheap-wins batch fixed (2026-07-17, `maturity-work` branch)** — findings #12, #13, #18 (global init half), #22, #36:
- **#12**: trap/parallel synthesized-function IDs now come from a shared monotonic `Cell<usize>` counter on `CodeGen` (`next_synth_id()`), replacing the O(n) `get_functions().count()` per site.
- **#13**: module name (file stem) and the `__llm_{hash}_` mangle prefix are computed once in `CodeGen::new` (`input_path` is construction-only) and cached as fields; `get_module_name()` now returns `&str`.
- **#18**: `llm_curl_ensure_init()` (a `pthread_once` around `curl_global_init`, defined in `http.c`, declared in `common.h`, shared by `http_server.c`) runs before every `curl_easy_init`, eliminating the multi-thread lazy-init race. Per-request handle churn (no connection/DNS/TLS-session reuse) remains open — medium, blocked on #23's dedup being the sane place to add a handle cache.
- **#22**: both workflows gained `concurrency:` groups (CI cancels superseded runs; release never cancels) and `Swatinem/rust-cache@v2` (skipped for the Alpine matrix entry, which builds inside docker).
- **#36**: `llm-clang` and `llm-test` now run under `set -eo pipefail` (`-u` deliberately omitted: macOS bash 3.2 trips on empty-array expansion); `llm-clang` aborts with a message if a runtime C compile or `ar` fails instead of silently linking stale objects, resolves Homebrew paths via `brew --prefix` (hardcoded paths only as fallback), and refreshes `./libllm_opencl.so` only when missing or stale; `llm-test` fails fast with the server log and port state when the mock server can't bind (previously a swallowed warning). The port is still fixed at 8080 because test files hardcode the URL — concurrent runs on one host fail loudly now, not silently.

**Fixed post-audit (2026-07-17, `maturity-work` branch)**: CI flakiness from live-internet dependence — `http_live_test.llm` and `https_json_test.llm` called httpbin.org, whose outages turned CI red (observed July 17). The `llm-test` mock server now provides httpbin-style JSON echo endpoints (`/json/get`, `/json/post`) and both tests target it; TLS client coverage remains with `https_test.llm`/`run_https_test.sh`.

**Checked and explicitly fine**: `llm-clang`'s incremental runtime rebuild works (`-nt` guards + build-mode cache; only the unconditional `ar rcs` is wasteful); no redundant IR emission in the compile paths; node20 action deprecations are latent (runtime already forced to node24 via `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24`).

---

## Recommended sequencing

1. **`display.rs` round-trip fixes + a parse→print→parse property test** (#1) — trivial fixes that unblock the MCP patch feature and should let `patch_test.llm` back into CI.
2. **Runtime safety trio** (#2 `fdopen`, #3 atomic refcounts, #6 MCP index/unwrap) — each small, each a real crash or corruption.
3. **Cheap-wins batch** (#12 trap-ID counter, #13 cached module name, #18 `curl_global_init`, #22 CI cache + concurrency, #36 `set -e` in scripts) — roughly an afternoon, measurable compile/CI speedup.
4. **Scheduled refactors**: `Result`-based codegen with real source spans (#7, also fixes #37's `line: 0` and the line-1 diagnostics), centralized pointer-policy metadata (#24), fd/pointer handle tagging (#4), runtime error-signaling convention (#31).

---

*Generated by a three-agent verified scan (compiler core, C runtime, tooling/interfaces) on 2026-07-16. Line numbers reference `main` @ `3ebb79d`.*
