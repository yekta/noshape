#!/usr/bin/env python3
"""Document open speed (PLAN 2.7: open → first viewport frame ≤ 0.5 s): each document opened in the release app
(`NOSHAPE_PERF=1 NOSHAPE_PERF_SCRIPT=open`, which quits once the full model's frame is up), the milestones read from
its log:

    open → model   "opening document" to the first frame drawing the model (the preview display: what users see first)
    open → full    to the frame drawing the full display (refined meshes)
    reopen → model the same opening again on the display cache the cold open wrote (the cached model's frame)
    regen          the regeneration of the preview / full display (its own clock)
    shown          the display coming in to its first frame (scene build, upload, the frame)

    tools/perf/open-times.py --app <noshape> --out <dir> [--runs 3] [--reopen 3] [--display :390] [--live /root/parity/models]
        <document.noshape | directory>...

Prints a markdown table (median of the runs) sorted slowest first, with live's full regeneration time
(live.jsonl `full_regen_ms`) where the corpus recorded one; writes <out>/open-times.json.
"""
import argparse
import json
import os
import re
import shutil
import statistics
import subprocess
import tempfile
import time
from pathlib import Path

MILESTONE = re.compile(r"(opening document|open: first regen started|open: (preview|full) display in \(regen ([0-9.]+) ms\)|open: (preview|full|cached) model frame presented|first viewport frame presented): ([0-9.]+) ms since start")


def run_one(app, doc, display, timeout, cache):
    work = Path(tempfile.mkdtemp(prefix="noshape-open-"))
    copy = work / doc.name
    # A document is SQLite: its write-ahead log goes with it.
    for ext in ("", "-wal", "-shm"):
        if Path(f"{doc}{ext}").exists():
            shutil.copy(f"{doc}{ext}", f"{copy}{ext}")
    env = dict(os.environ, NOSHAPE_PERF="1", NOSHAPE_PERF_SCRIPT="open", DISPLAY=display, NOSHAPE_CONFIG_DIR=str(work / "config"), NOSHAPE_NO_NATIVE_DIALOGS="1", RUST_LOG="info", NOSHAPE_CACHE_DIR=str(cache))
    t = {}
    try:
        p = subprocess.run([app, str(copy)], env=env, capture_output=True, text=True, timeout=timeout)
        log = p.stdout + p.stderr
    except subprocess.TimeoutExpired as e:
        text = lambda b: b.decode(errors="replace") if isinstance(b, bytes) else (b or "")
        log = text(e.stdout) + text(e.stderr)
        t["timeout"] = True
    finally:
        shutil.rmtree(work, ignore_errors=True)
    for m in MILESTONE.finditer(log):
        at = float(m.group(5))
        if m.group(1) == "opening document":
            t.setdefault("open", at)
        elif m.group(1) == "open: first regen started":
            t.setdefault("regen_start", at)
        elif m.group(2):
            t.setdefault(f"{m.group(2)}_in", at)
            t.setdefault(f"{m.group(2)}_regen", float(m.group(3)))
        elif m.group(4):
            t.setdefault(f"{m.group(4)}_frame", at)
        else:
            t.setdefault("first_frame", at)
    return t


def summary(runs):
    def med(key, base=None):
        vals = [r[key] - (r[base] if base else 0) for r in runs if key in r and (base is None or base in r)]
        return round(statistics.median(vals)) if vals else None
    # A studio whose regeneration needs no refine shows its full display first: "model" is then the full frame.
    model = med("preview_frame", "open")
    full = med("full_frame", "open")
    return {
        "model_ms": model if model is not None else full,
        "full_ms": full,
        "regen_ms": med("preview_regen") if med("preview_regen") is not None else med("full_regen"),
        "full_regen_ms": med("full_regen"),
        "shown_ms": med("preview_frame", "preview_in") if med("preview_frame", "preview_in") is not None else med("full_frame", "full_in"),
        "window_ms": med("first_frame", "open"),
        "regen_start_ms": med("regen_start", "open"),
        "timeouts": sum(1 for r in runs if r.get("timeout")),
    }


def reopen_summary(runs):
    """The reopens' median to the first frame with a model: the cached one, else the regenerated one."""
    vals = []
    for r in runs:
        at = r.get("cached_frame", r.get("preview_frame", r.get("full_frame")))
        if at is not None and "open" in r:
            vals.append(at - r["open"])
    return {"reopen_ms": round(statistics.median(vals)) if vals else None, "reopen_cached": sum(1 for r in runs if "cached_frame" in r)}


def live_regen(live_dir, name):
    f = Path(live_dir) / name / "live.jsonl"
    if not f.exists():
        return None
    for line in f.read_text().splitlines():
        try:
            v = json.loads(line).get("full_regen_ms")
        except json.JSONDecodeError:
            continue
        if v is not None:
            return round(v)
    return None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--app", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--runs", type=int, default=3, help="cold opens (each with an empty display cache)")
    ap.add_argument("--reopen", type=int, default=3, help="reopens on the last cold open's display cache")
    ap.add_argument("--display", default=":390")
    ap.add_argument("--timeout", type=int, default=180)
    ap.add_argument("--live", default="/root/parity/models")
    ap.add_argument("docs", nargs="+")
    a = ap.parse_args()
    docs = []
    for d in map(Path, a.docs):
        docs += sorted(d.glob("*.noshape")) if d.is_dir() else [d]
    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    rows = {}
    for doc in docs:
        runs = []
        for _ in range(a.runs):
            cache = Path(tempfile.mkdtemp(prefix="noshape-open-cache-"))
            runs.append(run_one(a.app, doc, a.display, a.timeout, cache))
            if len(runs) < a.runs:
                shutil.rmtree(cache, ignore_errors=True)
        again = [run_one(a.app, doc, a.display, a.timeout, cache) for _ in range(a.reopen)]
        shutil.rmtree(cache, ignore_errors=True)
        rows[doc.stem] = dict(summary(runs), **reopen_summary(again), live_regen_ms=live_regen(a.live, doc.stem), runs=runs, reopens=again)
        (out / "open-times.json").write_text(json.dumps(rows, indent=1))
        print(f"{doc.stem}: model {rows[doc.stem]['model_ms']} ms, full {rows[doc.stem]['full_ms']} ms, reopen {rows[doc.stem]['reopen_ms']} ms", flush=True)
    key = lambda kv: -(kv[1]["model_ms"] if kv[1]["model_ms"] is not None else 1e9)
    print("\n| document | open → model (ms) | reopen → model | open → full | regen starts | regen (preview) | regen (full) | display → frame | live regen |")
    print("|---|---|---|---|---|---|---|---|---|")
    for name, r in sorted(rows.items(), key=key):
        cell = lambda v: "–" if v is None else str(v)
        flag = " (timeout)" if r["timeouts"] else ""
        print(f"| {name}{flag} | {cell(r['model_ms'])} | {cell(r.get('reopen_ms'))}{'' if r.get('reopen_cached') else ' (no cache)'} | {cell(r['full_ms'])} | {cell(r.get('regen_start_ms'))} | {cell(r['regen_ms'])} | {cell(r['full_regen_ms'])} | {cell(r['shown_ms'])} | {cell(r['live_regen_ms'])} |")
    met = [r for r in rows.values() if r["model_ms"] is not None and r["model_ms"] <= 500]
    again = [r for r in rows.values() if r.get("reopen_ms") is not None and r["reopen_ms"] <= 500]
    print(f"\n{len(met)}/{len(rows)} documents show their model within 0.5 s of opening, {len(again)}/{len(rows)} of reopening")


if __name__ == "__main__":
    raise SystemExit(main())
