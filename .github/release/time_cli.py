"""time_cli.py <cli> <doc>...: each document opened through the CLI's MCP server (document_open, then
feature_list), best of three, in seconds; a table on stdout and in the job summary: the release's speed per platform
(green builds opened tests/perf/release-fillet-hang.noshape 1.5x faster than v0.4.32 only because their OCCT had
x86-64-v3 code; 11-06)."""
import json, os, shutil, subprocess, sys, tempfile, time


class Cli:
    def __init__(self, path):
        self.p = subprocess.Popen([path, "mcp"], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True, bufsize=1)
        self.i = 0
        self.rpc("initialize", {"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "time_cli", "version": "1"}})
        self.p.stdin.write(json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n")

    def rpc(self, method, params):
        self.i += 1
        self.p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": self.i, "method": method, "params": params}) + "\n")
        while True:
            line = self.p.stdout.readline()
            if not line:
                raise RuntimeError("the CLI exited")
            m = json.loads(line)
            if m.get("id") == self.i:
                return m

    def call(self, tool, args):
        r = self.rpc("tools/call", {"name": tool, "arguments": args})["result"]
        text = "".join(c.get("text", "") for c in r.get("content", []))
        if r.get("isError"):
            raise RuntimeError(f"{tool}: {text[:300]}")
        return json.loads(text)

    def close(self):
        self.p.stdin.close()
        self.p.wait(timeout=60)


def once(cli, doc, tmp):
    copy = os.path.join(tmp, "t-" + os.path.basename(doc))
    shutil.copy(doc, copy)
    c = Cli(cli)
    t = time.monotonic()
    c.call("document_open", {"path": copy})
    n = len(c.call("feature_list", {})["features"])
    dt = time.monotonic() - t
    c.close()
    os.remove(copy)
    return dt, n


def cpu():
    """The runner's processor and core count: hosted runners of one OS differ (Linux's fins-400 1.36 to 2.70 s
    between runs of the same code, 11-06), so a table is read against its hardware."""
    name = ""
    try:
        if sys.platform.startswith("linux"):
            with open("/proc/cpuinfo", encoding="utf-8") as f:
                name = next((l.split(":", 1)[1].strip() for l in f if l.startswith("model name")), "")
        elif sys.platform == "darwin":
            name = subprocess.run(["sysctl", "-n", "machdep.cpu.brand_string"], capture_output=True, text=True).stdout.strip()
        elif sys.platform == "win32":
            name = subprocess.run(["powershell", "-NoProfile", "-Command", "(Get-CimInstance Win32_Processor).Name"], capture_output=True, text=True).stdout.strip()
    except (OSError, StopIteration):
        pass
    return f"{name or 'unknown processor'}, {os.cpu_count()} cores"


def main():
    cli, docs = sys.argv[1], sys.argv[2:]
    hw = cpu()
    print(f"cpu: {hw}", flush=True)
    rows = []
    with tempfile.TemporaryDirectory() as tmp:
        for doc in docs:
            runs = [once(cli, doc, tmp) for _ in range(3)]
            rows.append((os.path.basename(doc), min(r[0] for r in runs), runs[0][1]))
            print(f"{rows[-1][0]}: {rows[-1][1]:.2f} s ({rows[-1][2]} features)", flush=True)
    table = "| document | open + feature list (best of 3) | features |\n|---|---|---|\n" + "".join(f"| {d} | {t:.2f} s | {n} |\n" for d, t, n in rows)
    summary = os.environ.get("GITHUB_STEP_SUMMARY")
    if summary:
        with open(summary, "a", encoding="utf-8") as f:
            f.write(f"### Regeneration times ({os.environ.get('RUNNER_OS', '?')}, {os.path.basename(cli)}; {hw})\n\n{table}\n")


if __name__ == "__main__":
    main()
