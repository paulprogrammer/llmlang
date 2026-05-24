#!/bin/bash
set -e

echo "Compiling HTTP Router test..."
./llm-clang tests/lang/http_router_test.llm -o tests/lang/http_router_test_bin

echo "Starting HTTP Router Server..."
tests/lang/http_router_test_bin > router_out.log 2>&1 &
SERVER_PID=$!

cleanup() {
    echo "Cleaning up..."
    kill $SERVER_PID >/dev/null 2>&1 || true
    rm -f tests/lang/http_router_test_bin router_out.log
}
trap cleanup EXIT

# Give the server a moment to start and bind
sleep 1.0

# 1. Test first route (GET /first)
echo "Testing GET /first..."
RES1=$(curl -s http://127.0.0.1:8082/first)
echo "Response: $RES1"
if [ "$RES1" != "Response from first handler" ]; then
    echo "FAIL: GET /first returned unexpected response: '$RES1'"
    exit 1
fi

# 2. Test second route (POST /second)
echo "Testing POST /second..."
RES2=$(curl -s -X POST http://127.0.0.1:8082/second)
echo "Response: $RES2"
if [ "$RES2" != '{"route":"second"}' ]; then
    echo "FAIL: POST /second returned unexpected response: '$RES2'"
    exit 1
fi

# 3. Test method mismatch (GET /second)
echo "Testing GET /second (method mismatch)..."
RES3=$(curl -s http://127.0.0.1:8082/second)
echo "Response: $RES3"
if [ "$RES3" != "404 Not Found" ]; then
    echo "FAIL: GET /second (method mismatch) did not return fallback: '$RES3'"
    exit 1
fi

# 4. Test path mismatch (GET /invalid)
echo "Testing GET /invalid..."
RES4=$(curl -s http://127.0.0.1:8082/invalid)
echo "Response: $RES4"
if [ "$RES4" != "404 Not Found" ]; then
    echo "FAIL: GET /invalid did not return fallback: '$RES4'"
    exit 1
fi

echo "All HTTP Router requests verified successfully!"
echo "Server logs:"
cat router_out.log

echo "PASS: HTTP Router integration test succeeded!"
