"""An installed noshape updates itself (release.yml's verify job): detect, download, verify, apply, relaunch.

  python check_update.py <installed program> <update asset> <signed dir> <signer identity> [--stub <file>] [--inode <path>]

A local HTTP server stands in for GitHub's releases/latest: a test release (this run's own build, tagged with its
version) with <update asset>, and SHA256SUMS and its Sigstore bundle from <signed dir> (release.yml's sign job). The
installed app is started with a document, told it is an older version (NOSHAPE_UPDATE_SELF_VERSION) and pointed at
the feed (NOSHAPE_UPDATE_FEED, NOSHAPE_UPDATE_TEST_IDENTITY: a dry-run build only), and restarts on its own once the
update is verified (NOSHAPE_UPDATE_AUTORESTART). Checked through its log (NOSHAPE_UPDATE_LOG):

1. the update is found, verified, applied by the helper, and the new version starts with the document reopened;
   the install changed: --inode <path> has a new inode (the swapped binary or bundle), --stub <file> (overwritten
   with a stub first) is a real file again;
2. the same release with its download changed on the way: refused (checksum);
3. the same release, the app expecting another signer (a release tag's): refused (signed by another workflow).

Exits 1 on any mismatch.
"""
import http.server
import json
import os
import shutil
import subprocess
import sys
import tempfile
import threading
import time

program, asset, signed, identity = sys.argv[1:5]
opts = sys.argv[5:]
stub = opts[opts.index("--stub") + 1] if "--stub" in opts else None
inode = opts[opts.index("--inode") + 1] if "--inode" in opts else None
name = os.path.basename(asset)
# "noshape-0.0.0-1a2b3c4-linux-x86_64.tar.gz" -> "0.0.0-1a2b3c4"
version = name[len("noshape-"):].split("-linux-")[0].split("-macos-")[0].split("-windows-")[0]
work = tempfile.mkdtemp(prefix="noshape-update-check-")
print(f"update check: {program}, release v{version} ({name}), signer {identity}")


def serve(root):
    """Serve `root`; the feed is root/latest. The server's base URL."""

    class Handler(http.server.SimpleHTTPRequestHandler):
        def __init__(self, *a, **k):
            super().__init__(*a, directory=root, **k)

        def log_message(self, *a):
            pass

    s = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    threading.Thread(target=s.serve_forever, daemon=True).start()
    return f"http://127.0.0.1:{s.server_address[1]}"


def feed(tag, tamper=False):
    root = tempfile.mkdtemp(prefix="feed-", dir=work)
    for f in [asset, os.path.join(signed, "SHA256SUMS"), os.path.join(signed, "SHA256SUMS.sigstore.json")]:
        shutil.copy(f, root)
    if tamper:
        with open(os.path.join(root, name), "r+b") as f:
            f.seek(os.path.getsize(asset) // 2)
            b = f.read(1)
            f.seek(-1, 1)
            f.write(bytes([b[0] ^ 1]))
    base = serve(root)
    files = [name, "SHA256SUMS", "SHA256SUMS.sigstore.json"]
    release = {"tag_name": tag, "body": "The update check's release.", "html_url": f"{base}/release",
               "assets": [{"name": n, "browser_download_url": f"{base}/{n}", "size": os.path.getsize(os.path.join(root, n))} for n in files]}
    with open(os.path.join(root, "latest"), "w") as f:
        json.dump(release, f)
    return f"{base}/latest"


def run(label, url, signer, expect, autorestart=False, doc=None, timeout=600):
    """Start the app on the feed `url`; wait for a log line containing every string of `expect`, in order. The log."""
    log = os.path.join(work, f"{label}.log")
    config = os.path.join(work, f"config-{label}")
    os.makedirs(config)
    env = dict(os.environ, NOSHAPE_UPDATE_FEED=url, NOSHAPE_UPDATE_SELF_VERSION="0.0.0-old", NOSHAPE_UPDATE_TEST_IDENTITY=signer,
               NOSHAPE_UPDATE_LOG=log, NOSHAPE_CONFIG_DIR=config, NOSHAPE_NO_SETUP_PROMPT="1", NOSHAPE_NO_NATIVE_DIALOGS="1")
    env.pop("NOSHAPE_NO_UPDATES", None)
    if autorestart:
        env["NOSHAPE_UPDATE_AUTORESTART"] = "1"
    app = subprocess.Popen([program] + ([doc] if doc else []), env=env, stdin=subprocess.DEVNULL)
    # Reaped when it quits, as a desktop or a shell does.
    threading.Thread(target=app.wait, daemon=True).start()
    start, text, want = time.time(), "", list(expect)
    while want and time.time() - start < timeout:
        time.sleep(1)
        text = open(log, encoding="utf-8", errors="replace").read() if os.path.exists(log) else ""
        want = list(expect)
        for line in text.splitlines():
            while want and want[0] in line:
                want.pop(0)
    print(f"--- {label} ({time.time() - start:.0f} s):\n{text}---")
    pids = [int(l.split()[1].rstrip(":")) for l in text.splitlines() if l.startswith("app ") and ": started " in l]
    for pid in set(pids + [app.pid]):
        kill(pid)
    if want:
        sys.exit(f"{label}: no log line with {want[0]!r} after {timeout} s")
    return text


def kill(pid):
    if os.name == "nt":
        subprocess.run(["taskkill", "/F", "/T", "/PID", str(pid)], capture_output=True)
    else:
        try:
            os.kill(pid, 9)
        except OSError:
            pass


# A document, open when the update restarts the app.
doc = os.path.join(work, "update check.noshape")
r = subprocess.run([program, "call", "document_create", json.dumps({"path": doc})], capture_output=True, text=True, timeout=300)
if r.returncode != 0 or not os.path.exists(doc):
    sys.exit(f"document_create: exit {r.returncode}: {r.stdout} {r.stderr}")
if stub:
    with open(stub, "wb") as f:
        f.write(b"stub")
before = os.stat(inode).st_ino if inode else None

tag = f"v{version}"
text = run("update", feed(tag), identity, ["found " + version, "verified " + version, "restarting into " + version + " with 1 document(s)",
                                           "helper: swapped", f"started {version}", "reopened 1 document(s)"], autorestart=True, doc=doc)
if inode and os.stat(inode).st_ino == before:
    sys.exit(f"{inode} wasn't replaced")
if stub and open(stub, "rb").read() == b"stub":
    sys.exit(f"{stub} wasn't replaced")
run("tampered", feed(tag, tamper=True), identity, ["found " + version, "refused " + version, "checksum"])
other = f"https://github.com/yekta/noshape/.github/workflows/release.yml@refs/tags/{tag}"
run("other-signer", feed(tag), other, ["found " + version, "refused " + version, "signed by"])
print("OK")
