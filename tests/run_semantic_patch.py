import subprocess
import json
import time
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

# 0. Initialize
init_req = {
    "jsonrpc": "2.0",
    "id": 0,
    "method": "initialize",
    "params": {
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": {"name": "test", "version": "1.0"}
    }
}
proc.stdin.write(json.dumps(init_req) + "\n")
proc.stdin.flush()
print("Sent initialize:", proc.stdout.readline().strip())

init_notif = {
    "jsonrpc": "2.0",
    "method": "notifications/initialized"
}
proc.stdin.write(json.dumps(init_notif) + "\n")
proc.stdin.flush()

# 1. Analyze the codebase
analyze_req = {
    "jsonrpc": "2.0",
    "id": 1,
    "method": "tools/call",
    "params": {
        "name": "analyze_codebase",
        "arguments": {
            "path": "tests/lang"
        }
    }
}
proc.stdin.write(json.dumps(analyze_req) + "\n")
proc.stdin.flush()
print("Sent analyze_codebase")
print("Response:", proc.stdout.readline().strip())

# 2. Patch the main function
# We will replace `+ $ x $ y` with `* $ x $ y`
patch_req = {
    "jsonrpc": "2.0",
    "id": 2,
    "method": "tools/call",
    "params": {
        "name": "patch_symbol",
        "arguments": {
            "function_name": "main",
            "new_body_ast": {
                "BinaryOp": [
                    "Mul",
                    { "Borrow": { "Identifier": "x" } },
                    { "Borrow": { "Identifier": "y" } }
                ]
            }
        }
    }
}
proc.stdin.write(json.dumps(patch_req) + "\n")
proc.stdin.flush()
print("Sent patch_symbol")
print("Response:", proc.stdout.readline().strip())

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
