import subprocess
import json
import os

# Create a dummy .llm file for patching
os.makedirs("tests/lang", exist_ok=True)
dummy_code = """
# Point x y
: main x y
    + $ x $ y
"""
with open("tests/lang/patch_test.llm", "w") as f:
    f.write(dummy_code.strip() + "\n")

# Start MCP Server
proc = subprocess.Popen(["cargo", "run", "--bin", "llm-mcp"], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)


def rpc(request):
    proc.stdin.write(json.dumps(request) + "\n")
    proc.stdin.flush()
    if "id" in request:
        return proc.stdout.readline().strip()


# 0. Initialize
print("Sent initialize:", rpc({
    "jsonrpc": "2.0",
    "id": 0,
    "method": "initialize",
    "params": {
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": {"name": "test", "version": "1.0"}
    }
}))
rpc({"jsonrpc": "2.0", "method": "notifications/initialized"})

# 1. Analyze the codebase (tests/lang defines `main` in many files)
print("analyze_codebase:", rpc({
    "jsonrpc": "2.0",
    "id": 1,
    "method": "tools/call",
    "params": {
        "name": "analyze_codebase",
        "arguments": {"path": "tests/lang"}
    }
}))

new_body = {
    "BinaryOp": [
        "Mul",
        {"Borrow": {"Identifier": "x"}},
        {"Borrow": {"Identifier": "y"}}
    ]
}

# 2. Patching an ambiguous name without a path must be rejected,
#    not silently rewrite whichever file happened to win the index.
ambiguous = rpc({
    "jsonrpc": "2.0",
    "id": 2,
    "method": "tools/call",
    "params": {
        "name": "patch_symbol",
        "arguments": {"function_name": "main", "new_body_ast": new_body}
    }
})
print("ambiguous patch:", ambiguous)
if "error" not in json.loads(ambiguous) or "multiple files" not in ambiguous:
    print("FAILED: ambiguous patch_symbol was not rejected")
    proc.terminate()
    exit(1)

# 3. Patch with the path to disambiguate:
#    replace `+ $ x $ y` with `* $ x $ y`
print("patch_symbol:", rpc({
    "jsonrpc": "2.0",
    "id": 3,
    "method": "tools/call",
    "params": {
        "name": "patch_symbol",
        "arguments": {
            "function_name": "main",
            "path": "tests/lang/patch_test.llm",
            "new_body_ast": new_body
        }
    }
}))

# Close server
proc.terminate()

# Check file contents
with open("tests/lang/patch_test.llm", "r") as f:
    result = f.read()

print("\n--- Patched Source Code ---")
print(result)

if "* $ x $ y" in result:
    print("SUCCESS: Semantic Patching worked!")
    exit(0)
else:
    print("FAILED: Source code did not reflect AST patch")
    exit(1)
