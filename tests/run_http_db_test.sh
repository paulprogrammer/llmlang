#!/bin/bash
set -e

# Setup mock Kubernetes Service Binding
mkdir -p tests/bindings/db-primary
echo "test.db" > tests/bindings/db-primary/database
export SERVICE_BINDING_ROOT="$(pwd)/tests/bindings"

echo "Compiling HTTP Database test..."
./llm-clang tests/lang/http_db_test.llm -o tests/lang/http_db_test_bin

echo "Starting HTTP Database Server..."
tests/lang/http_db_test_bin > db_test_out.log 2>&1 &
SERVER_PID=$!

cleanup() {
    echo "Cleaning up..."
    kill $SERVER_PID >/dev/null 2>&1 || true
    if [ -f db_test_out.log ]; then
        echo "=== SERVER LOGS ==="
        cat db_test_out.log
        echo "==================="
    fi
    rm -rf tests/lang/http_db_test_bin db_test_out.log tests/bindings test.db
}
trap cleanup EXIT

# Give the server a moment to start and bind
sleep 1.5

echo "Querying HTTP Database Server..."
RES=$(curl -s http://127.0.0.1:8085/test-db)
echo "Response: $RES"

EXPECTED="SQLite Bob: Bob, Redis: mock_value, Mongo: mock_id_123, K8s Count: 1"
if [ "$RES" != "$EXPECTED" ]; then
    echo "FAIL: unexpected response: '$RES'"
    echo "Expected: '$EXPECTED'"
    echo "Server logs:"
    cat db_test_out.log
    exit 1
fi

echo "All Database Connectors test cases verified successfully!"
echo "Server logs:"
cat db_test_out.log

echo "PASS: HTTP Database integration test succeeded!"
