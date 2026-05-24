#!/bin/bash
set -e

echo "Compiling HTTP Headers test..."
./llm-clang tests/lang/http_headers_test.llm -o tests/lang/http_headers_test_bin

echo "Starting HTTP Headers Server..."
tests/lang/http_headers_test_bin > headers_test_out.log 2>&1 &
SERVER_PID=$!

cleanup() {
    echo "Cleaning up..."
    kill $SERVER_PID >/dev/null 2>&1 || true
    rm -f tests/lang/http_headers_test_bin headers_test_out.log
}
trap cleanup EXIT

# Give the server a moment to start and bind
sleep 1.5

# 1. Standard Header extraction
echo "Testing standard Authorization header..."
RES1=$(curl -s -H "Authorization: Bearer secret123" http://127.0.0.1:8084/test-headers)
echo "Response: $RES1"
if [ "$RES1" != "Auth: Bearer secret123, Key: , Missing: " ]; then
    echo "FAIL: unexpected response for standard header: '$RES1'"
    exit 1
fi

# 2. Case-insensitive header extraction
echo "Testing case-insensitive x-custom-key header..."
RES2=$(curl -s -H "x-custom-key: value456" http://127.0.0.1:8084/test-headers)
echo "Response: $RES2"
if [ "$RES2" != "Auth: , Key: value456, Missing: " ]; then
    echo "FAIL: unexpected response for case-insensitive header: '$RES2'"
    exit 1
fi

# 3. Missing header
echo "Testing missing header..."
RES3=$(curl -s http://127.0.0.1:8084/test-headers)
echo "Response: $RES3"
if [ "$RES3" != "Auth: , Key: , Missing: " ]; then
    echo "FAIL: unexpected response for missing header: '$RES3'"
    exit 1
fi

# 4. Multiple headers combined
echo "Testing multiple headers combined..."
RES4=$(curl -s -H "Authorization: Bearer secret123" -H "X-Custom-Key: value456" http://127.0.0.1:8084/test-headers)
echo "Response: $RES4"
if [ "$RES4" != "Auth: Bearer secret123, Key: value456, Missing: " ]; then
    echo "FAIL: unexpected response for combined headers: '$RES4'"
    exit 1
fi

echo "All HTTP Headers test cases verified successfully!"
echo "Server logs:"
cat headers_test_out.log

echo "PASS: HTTP Headers integration test succeeded!"
