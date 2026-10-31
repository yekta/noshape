"""A release binary answers as the CLI and the MCP server (release.yml's verify job).

  python check_binary.py <program> [<expected --version name>]

Runs `<program> --version` (prints "<name> <version>"), then `<program> mcp` as an MCP client does: initialize,
notifications/initialized and tools/list over stdin/stdout pipes, one JSON message per line. Exits 1 on any mismatch.
"""
import json
import subprocess
import sys

program = sys.argv[1]
name = sys.argv[2] if len(sys.argv) > 2 else "noshape"

v = subprocess.run([program, "--version"], capture_output=True, text=True, timeout=120)
print(f"{program} --version: {v.stdout.strip()!r} (exit {v.returncode})")
if v.returncode != 0 or not v.stdout.startswith(name + " "):
    sys.exit(f"--version: expected '{name} <version>', got {v.stdout!r} {v.stderr!r}")

messages = [
    {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "release-check", "version": "1"}}},
    {"jsonrpc": "2.0", "method": "notifications/initialized"},
    {"jsonrpc": "2.0", "id": 2, "method": "tools/list"},
]
p = subprocess.run([program, "mcp"], input="".join(json.dumps(m) + "\n" for m in messages), capture_output=True, text=True, timeout=300)
replies = [json.loads(line) for line in p.stdout.splitlines() if line.strip()]
if p.returncode != 0 or len(replies) != 2:
    sys.exit(f"mcp: exit {p.returncode}, {len(replies)} replies; stderr: {p.stderr[-2000:]}")
info = replies[0]["result"]["serverInfo"]
tools = [t["name"] for t in replies[1]["result"]["tools"]]
print(f"mcp: {info['name']} {info['version']}, {len(tools)} tools")
if info["name"] != "noshape" or "feature_add" not in tools or "ops_call" not in tools:
    sys.exit(f"mcp: unexpected replies {replies[0]} / {tools}")
print("OK")
