#!/usr/bin/env python3
"""Minimal MCP stdio client for figcraft-cli.

Usage:
  python3 figcraft_mcp.py list
  python3 figcraft_mcp.py call <tool> '<json args>'
  python3 figcraft_mcp.py script <file.json>     # [[tool, args], ...] in one session
"""

import json
import subprocess
import sys
import threading

ROOT = "/home/bmoser/repos/canonwright/design"


class Client:
    def __init__(self):
        self.proc = subprocess.Popen(
            ["figcraft-cli", "mcp", "--automation-write-root", ROOT],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1,
        )
        self.next_id = 1
        self.stderr = []
        threading.Thread(target=self._drain, daemon=True).start()
        self.request(
            "initialize",
            {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "canonwright-design", "version": "0.1"},
            },
        )
        self.notify("notifications/initialized", {})

    def _drain(self):
        for line in self.proc.stderr:
            self.stderr.append(line.rstrip())

    def _send(self, payload):
        self.proc.stdin.write(json.dumps(payload) + "\n")
        self.proc.stdin.flush()

    def notify(self, method, params):
        self._send({"jsonrpc": "2.0", "method": method, "params": params})

    def request(self, method, params):
        request_id = self.next_id
        self.next_id += 1
        self._send({"jsonrpc": "2.0", "id": request_id, "method": method, "params": params})
        while True:
            line = self.proc.stdout.readline()
            if not line:
                raise RuntimeError("server closed: " + "\n".join(self.stderr[-8:]))
            line = line.strip()
            if not line:
                continue
            try:
                message = json.loads(line)
            except json.JSONDecodeError:
                continue
            if message.get("id") != request_id:
                continue
            if "error" in message:
                raise RuntimeError(json.dumps(message["error"]))
            return message.get("result")

    def close(self):
        try:
            self.proc.stdin.close()
        except Exception:
            pass
        self.proc.terminate()


def main():
    mode = sys.argv[1] if len(sys.argv) > 1 else "list"
    client = Client()
    try:
        if mode == "list":
            result = client.request("tools/list", {})
            for tool in result.get("tools", []):
                print(tool["name"], "—", (tool.get("description") or "").split("\n")[0][:110])
            print(f"\n{len(result.get('tools', []))} tools")
        elif mode == "call":
            name = sys.argv[2]
            args = json.loads(sys.argv[3]) if len(sys.argv) > 3 else {}
            result = client.request("tools/call", {"name": name, "arguments": args})
            print(json.dumps(result, indent=1)[:4000])
        elif mode == "script":
            steps = json.loads(open(sys.argv[2]).read())
            for index, step in enumerate(steps):
                name, args = step[0], (step[1] if len(step) > 1 else {})
                result = client.request("tools/call", {"name": name, "arguments": args})
                content = result.get("content") or []
                text = content[0].get("text") if content else ""
                print(index, name, (text or "")[:160].replace("\n", " "))
    finally:
        client.close()


if __name__ == "__main__":
    main()
