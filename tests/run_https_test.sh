#!/bin/bash
set -e

# Compile the server test
./llm-clang tests/lang/https_test.llm -o tests/lang/https_test_bin

# Start the server
if command -v valgrind >/dev/null 2>&1; then
    echo "Running HTTPS Server test under Valgrind..."
    valgrind --leak-check=full --error-exitcode=99 tests/lang/https_test_bin > server_out.log 2>&1 &
    sleep 2
else
    echo "Starting HTTPS Server..."
    tests/lang/https_test_bin > server_out.log 2>&1 &
    sleep 0.5
fi
SERVER_PID=$!

cleanup() {
    kill $SERVER_PID >/dev/null 2>&1 || true
    rm -f tests/lang/https_test_bin server_out.log
}
trap cleanup EXIT

# Send first request
echo "Sending first request..."
RES1=$(curl -k -s https://127.0.0.1:8443/first_path)
echo "Response 1: $RES1"

# Send second request
echo "Sending second request..."
RES2=$(curl -k -s https://127.0.0.1:8443/second_path)
echo "Response 2: $RES2"

# Wait for server to exit
wait $SERVER_PID
EXIT_CODE=$?

# Check logs
echo "Server output logs:"
cat server_out.log

# Verify exit code
if [ $EXIT_CODE -ne 0 ] && [ $EXIT_CODE -ne 99 ]; then
    echo "FAIL: Server exited with non-zero code $EXIT_CODE"
    exit 1
fi

if [ $EXIT_CODE -eq 99 ]; then
    echo "FAIL: Valgrind detected memory leaks/errors"
    exit 1
fi

# Verify responses
if [ "$RES1" != "Hello from llmlang HTTPS server" ]; then
    echo "FAIL: Response 1 mismatch"
    exit 1
fi

if [ "$RES2" != '{"status":"ok"}' ]; then
    echo "FAIL: Response 2 mismatch"
    exit 1
fi

if ! grep -q "Received HTTPS request for: /first_path" server_out.log; then
    echo "FAIL: Log did not contain first path"
    exit 1
fi

if ! grep -q "Received HTTPS request for: /second_path" server_out.log; then
    echo "FAIL: Log did not contain second path"
    exit 1
fi

echo "PASS: HTTPS Server integration test succeeded!"
