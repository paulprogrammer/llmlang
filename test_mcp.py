import json
import subprocess
import sys

def run_test():
    proc = subprocess.Popen(['./target/debug/llm-mcp'], 
                            stdin=subprocess.PIPE, 
                            stdout=subprocess.PIPE, 
                            stderr=subprocess.PIPE,
                            text=True)

    def send(msg):
        line = json.dumps(msg)
        proc.stdin.write(line + '\n')
        proc.stdin.flush()

    def recv():
        return proc.stdout.readline()

    # 1. Initialize
    send({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "test-client",
                "version": "1.0.0"
            }
        }
    })
    init_resp = recv()
    print(f"Init Resp: {init_resp}")

    # 2. Initialized notification
    send({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    })

    # 3. tools/list
    send({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list"
    })
    tools_resp = recv()
    print(f"Tools Resp: {tools_resp}")

    proc.terminate()

if __name__ == "__main__":
    run_test()
