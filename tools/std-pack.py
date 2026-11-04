#!/usr/bin/env python3
"""Pack an older FeatureScript standard library for crates/featurescript/std-versions/<version>.zst.

usage: tools/std-pack.py <version> <dir with the release's *.fs>   (e.g. /root/onshape-research/live/std-1024)
       tools/std-pack.py --all                                      (every live/std-<v> fetched from Onshape)

A release is its modules in one blob (each `\\0<name>\\0<byte length>\\0<source>`, sorted by name), compressed
by zstd with the embedded std (crates/featurescript/std, the same blob of its modules) as the prefix: the
releases share most of their text with it, so each packs to ~50 KB (crates/featurescript/src/lib.rs unpacks
them the same way). Repack every release when crates/featurescript/std changes (a test fails until then).
"""
import os
import re
import subprocess
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
STD = os.path.join(ROOT, "crates/featurescript/std")
OUT = os.path.join(ROOT, "crates/featurescript/std-versions")
LIVE = "/root/onshape-research/live"


def blob(d):
    out = bytearray()
    for n in sorted(os.listdir(d)):
        if n.endswith(".fs"):
            b = open(os.path.join(d, n), "rb").read()
            out += f"\0{n}\0{len(b)}\0".encode() + b
    return bytes(out)


def pack(version, d):
    with tempfile.TemporaryDirectory() as t:
        prefix, src = os.path.join(t, "prefix"), os.path.join(t, "src")
        open(prefix, "wb").write(blob(STD))
        open(src, "wb").write(blob(d))
        dst = os.path.join(OUT, f"{version}.zst")
        subprocess.run(["zstd", "-q", "-f", "-19", "--long=27", f"--patch-from={prefix}", src, "-o", dst], check=True)
        print(f"{version}: {os.path.getsize(src)} bytes -> {os.path.getsize(dst)}")


def main():
    os.makedirs(OUT, exist_ok=True)
    # The embedded release itself (crates/featurescript/std) isn't packed.
    embedded = int(re.search(r"FeatureScript (\d+);", open(os.path.join(STD, "geometry.fs")).read()).group(1))
    if sys.argv[1:] == ["--all"]:
        for n in sorted(os.listdir(LIVE)):
            if n.startswith("std-") and n[4:].isdigit() and int(n[4:]) != embedded:
                pack(int(n[4:]), os.path.join(LIVE, n))
        return
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    pack(int(sys.argv[1]), sys.argv[2])


if __name__ == "__main__":
    main()
