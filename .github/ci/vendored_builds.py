"""Keep the vendored crates' builds between CI runs (ci-os.yml).

The vendored, patched crates (vendor/: gpui, wgpu-hal…) are path dependencies, so rust-cache drops their builds
like noshape's own (it keeps registry and git crates only), and every run rebuilt them: ~18 CPU-minutes on Windows,
the gpui chain the UI crates wait for. `save` moves their outputs out of target/ (into target-vendored/, which
rust-cache caches as an extra directory) before rust-cache cleans up; `restore` puts them back before the build.
Cargo reuses them because the vendored files get a fixed date after checkout and the cache is keyed on vendor/.

  python3 vendored_builds.py save|restore
"""
import json
import os
import shutil
import subprocess
import sys

SIDE = "target-vendored"
mode = sys.argv[1]
if mode == "restore":
    src, dst = SIDE, "target"
else:
    meta = json.loads(subprocess.run(["cargo", "metadata", "--format-version", "1"], capture_output=True, check=True).stdout)
    vendor = os.path.join(meta["workspace_root"], "vendor") + os.sep
    names = set()
    for p in meta["packages"]:
        if os.path.abspath(p["manifest_path"]).startswith(vendor):
            for n in [p["name"], *(t["name"] for t in p["targets"])]:
                names.update({n, n.replace("-", "_"), "lib" + n.replace("-", "_")})
    src, dst = "target", SIDE
moved = 0
if os.path.isdir(src):
    for profile in os.listdir(src):
        for sub in ("deps", ".fingerprint", "build"):
            d = os.path.join(src, profile, sub)
            if not os.path.isdir(d):
                continue
            for e in os.listdir(d):
                if mode == "save" and e.rsplit("-", 1)[0] not in names:
                    continue
                to = os.path.join(dst, profile, sub, e)
                os.makedirs(os.path.dirname(to), exist_ok=True)
                if os.path.exists(to):
                    shutil.rmtree(to) if os.path.isdir(to) else os.remove(to)
                shutil.move(os.path.join(d, e), to)
                moved += 1
print(f"{mode}: moved {moved} vendored build outputs ({src} -> {dst})")
