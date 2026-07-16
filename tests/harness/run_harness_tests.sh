#!/bin/bash
# Validation suite for the llmlang unified test harness (`llmlang test` + MCP run_symbol_tests).
# Run from the repo root: ./tests/harness/run_harness_tests.sh

set -u
cd "$(dirname "$0")/../.."

LLMLANG=./target/debug/llmlang
LLM_MCP=./target/debug/llm-mcp
RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m'
PASSED=0
FAILED=0

check() {
    local desc="$1"; shift
    if "$@" >/dev/null 2>&1; then
        echo -e "${GREEN}[PASS]${NC} $desc"
        PASSED=$((PASSED + 1))
    else
        echo -e "${RED}[FAIL]${NC} $desc"
        FAILED=$((FAILED + 1))
    fi
}

expect_exit() {
    local expected="$1"; shift
    "$@" >/dev/null 2>&1
    [ "$?" -eq "$expected" ]
}

cargo build 2>/dev/null || { echo "cargo build failed"; exit 1; }

echo "== Core engine unit tests =="
check "cargo unit tests (discovery, E019, harness synthesis)" cargo test --lib

echo "== Success path =="
check "pass suite exits 0 with explicit --test-data-dir" \
    expect_exit 0 $LLMLANG test tests/harness/pass_suite.llm --test-data-dir tests/harness/data
check "pass suite exits 0 with default ./tests/data fallback" \
    expect_exit 0 $LLMLANG test tests/harness/pass_suite.llm

echo "== Failure path =="
check "fail suite exits 1" \
    expect_exit 1 $LLMLANG test tests/harness/fail_suite.llm --test-data-dir tests/harness/data
OUT=$($LLMLANG test tests/harness/fail_suite.llm --test-data-dir tests/harness/data 2>&1)
check "exact panic message recorded" grep -q "Expected failure" <<<"$OUT"
check "missing data file recorded as graceful test failure" grep -q "cannot open test data file" <<<"$OUT"
check "isolated test passes after prior failures" grep -q "PASS test_isolated_env" <<<"$OUT"

echo "== Empty suite =="
OUT=$($LLMLANG test tests/harness/no_tests.llm 2>&1); RC=$?
check "empty suite exits 0" test "$RC" -eq 0
check "empty suite reports '0 tests found'" grep -q "0 tests found" <<<"$OUT"

echo "== Compiler diagnostics =="
OUT=$($LLMLANG test tests/harness/malformed.llm 2>&1); RC=$?
check "malformed M \"test\" target raises E019 and exits 1" \
    bash -c "[ $RC -eq 1 ] && grep -q E019 <<<'$OUT'"
OUT=$($LLMLANG test tests/harness/affine_violation.llm 2>&1); RC=$?
check "affine double-move rejected before execution (E004/E005)" \
    bash -c "[ $RC -eq 1 ] && grep -qE 'E004|E005' <<<'$OUT'"

echo "== Production stripping =="
STRIPPED=$($LLMLANG tests/harness/pass_suite.llm | grep -c "test_add_ok\|test_data_load")
check "test symbols absent from production IR" test "$STRIPPED" -eq 0

echo "== CLI format parity =="
TEXT_OUT=$($LLMLANG test tests/harness/fail_suite.llm --test-data-dir tests/harness/data --format=text 2>&1)
JSON_OUT=$($LLMLANG test tests/harness/fail_suite.llm --test-data-dir tests/harness/data --format=json 2>&1)
PARITY=$(python3 - "$TEXT_OUT" "$JSON_OUT" <<'EOF'
import json, re, sys
text, raw = sys.argv[1], sys.argv[2]
data = json.loads(raw)
results = {r["name"]: r for f in data["files"] for r in f["results"]}
ok = data["total"] == 3 and data["failed"] == 2 and data["passed"] == 1
for name, r in results.items():
    status = "PASS" if r["passed"] else "FAIL"
    ok &= bool(re.search(rf"{status} {name} ", text))
    if not r["passed"]:
        ok &= r["panic_message"] in text
m = re.search(r"Summary: (\d+) passed, (\d+) failed, (\d+) total", text)
ok &= m is not None and (int(m[1]), int(m[2]), int(m[3])) == (data["passed"], data["failed"], data["total"])
print("OK" if ok else "MISMATCH")
EOF
)
check "text and json represent identical data payloads" test "$PARITY" = "OK"

echo "== MCP run_symbol_tests =="
MCP_OUT=$({ printf '%s\n' \
    '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"t","version":"0"}}}' \
    '{"jsonrpc":"2.0","method":"notifications/initialized"}' \
    '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"run_symbol_tests","arguments":{"path":"tests/harness/fail_suite.llm","test_data_dir":"tests/harness/data"}}}'; \
    sleep 8; } | timeout 30 $LLM_MCP 2>/dev/null | tail -1)
MCP_OK=$(python3 - "$MCP_OUT" <<'EOF'
import json, sys
try:
    payload = json.loads(json.loads(sys.argv[1])["result"]["content"][0]["text"])
    fps = payload["failures"]
    ok = payload["failed"] == 2 and len(fps) == 2
    ok &= all(len(fp) == 64 for fp in fps)
    ok &= {v["symbol"] for v in fps.values()} == {"test_expected_failure", "test_missing_data"}
    ok &= any(v["panic_message"] == "Expected failure" for v in fps.values())
    print("OK" if ok else "MISMATCH")
except Exception as e:
    print(f"ERROR: {e}")
EOF
)
check "failures mapped to 64-char AST fingerprints with exact messages" test "$MCP_OK" = "OK"

echo "------------------------------------"
echo -e "Summary: ${GREEN}$PASSED passed${NC}, ${RED}$FAILED failed${NC}"
[ $FAILED -eq 0 ]
