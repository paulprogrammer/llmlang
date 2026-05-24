#!/bin/bash
set -e

echo "Compiling HTTP Dynamic Router test..."
./llm-clang tests/lang/http_dynamic_router_test.llm -o tests/lang/http_dynamic_router_test_bin

echo "Starting HTTP Dynamic Router Server..."
tests/lang/http_dynamic_router_test_bin > dynamic_router_out.log 2>&1 &
SERVER_PID=$!

cleanup() {
    echo "Cleaning up..."
    kill $SERVER_PID >/dev/null 2>&1 || true
    rm -f tests/lang/http_dynamic_router_test_bin dynamic_router_out.log
}
trap cleanup EXIT

# Give the server a moment to start and bind
sleep 1.5

# 1. Dynamic Path Parameter (Single)
echo "Testing GET /user/456..."
RES1=$(curl -s http://127.0.0.1:8083/user/456)
echo "Response: $RES1"
if [ "$RES1" != "User profile: 456" ]; then
    echo "FAIL: GET /user/456 returned unexpected response: '$RES1'"
    exit 1
fi

# 2. Dynamic Path Parameters (Multiple)
echo "Testing POST /posts/12/comments/34..."
RES2=$(curl -s -X POST http://127.0.0.1:8083/posts/12/comments/34)
echo "Response: $RES2"
if [ "$RES2" != "Post: 12, Comment: 34" ]; then
    echo "FAIL: POST /posts/12/comments/34 returned unexpected response: '$RES2'"
    exit 1
fi

# 3. Query Parameter Parsing & URL Decoding (%20)
echo "Testing GET /search?q=foo%20bar..."
RES3=$(curl -s "http://127.0.0.1:8083/search?q=foo%20bar")
echo "Response: $RES3"
if [ "$RES3" != "Search results for: foo bar" ]; then
    echo "FAIL: GET /search?q=foo%20bar returned unexpected response: '$RES3'"
    exit 1
fi

# 4. Query Parameter Parsing & URL Decoding (+)
echo "Testing GET /search?q=foo+bar..."
RES4=$(curl -s "http://127.0.0.1:8083/search?q=foo+bar")
echo "Response: $RES4"
if [ "$RES4" != "Search results for: foo bar" ]; then
    echo "FAIL: GET /search?q=foo+bar returned unexpected response: '$RES4'"
    exit 1
fi

# 5. Query Parameter Fallback (Empty query)
echo "Testing GET /search (no query)..."
RES5=$(curl -s "http://127.0.0.1:8083/search")
echo "Response: $RES5"
if [ "$RES5" != "Search results for: " ]; then
    echo "FAIL: GET /search (no query) returned unexpected response: '$RES5'"
    exit 1
fi

# 6. Dynamic Path Parameter combined with Query Parameter & URL Decoding
echo "Testing GET /groups/789?search=hello%20world..."
RES6=$(curl -s "http://127.0.0.1:8083/groups/789?search=hello%20world")
echo "Response: $RES6"
if [ "$RES6" != "Group: 789, Search: hello world" ]; then
    echo "FAIL: GET /groups/789?search=hello%20world returned unexpected response: '$RES6'"
    exit 1
fi

# 7. Exact Route matching with query present
echo "Testing GET /home?session=active..."
RES7=$(curl -s "http://127.0.0.1:8083/home?session=active")
echo "Response: $RES7"
if [ "$RES7" != "Home Page" ]; then
    echo "FAIL: GET /home?session=active returned unexpected response: '$RES7'"
    exit 1
fi

echo "All HTTP Dynamic Router and Query Parameter test cases verified successfully!"
echo "Server logs:"
cat dynamic_router_out.log

echo "PASS: HTTP Dynamic Router integration test succeeded!"
