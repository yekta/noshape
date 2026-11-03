#!/usr/bin/env python3
"""Interaction parity suite runner (tests/ui-parity/README.md).

Runs scenarios in noshape (NOSHAPE_UI_SCRIPT), then compares each recording with
live's (tests/ui-parity/live/<name>.json) step by step and names the first step
and key that differ. Prints one line per scenario and the score.

  tools/ui-parity/run.py --app $CARGO_TARGET_DIR/debug/noshape [--display :102] [names...]
  tools/ui-parity/run.py --compare-only            # recordings already in --out

Without names: every scenario that has a live recording (--all: every scenario).
On X11 without a window manager the app's window is focused with xdotool, so
fields get focus events as on a desktop.
"""

import argparse
import json
import os
import subprocess
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
KEYS = ["focus", "dialog", "list", "selection", "features", "parts", "message", "tooltip"]


def run_one(app, scenario, out, display, timeout):
    env = dict(os.environ)
    env.update({
        "NOSHAPE_UI_SCRIPT": str(scenario),
        "NOSHAPE_UI_STATE_OUT": str(out),
        "NOSHAPE_CONFIG_DIR": tempfile.mkdtemp(prefix="noshape-uip-config-"),
        "NOSHAPE_NO_NATIVE_DIALOGS": "1",
        "DISPLAY": display,
    })
    if out.exists():
        out.unlink()
    proc = subprocess.Popen([app], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True)
    t = time.time()
    while proc.poll() is None and time.time() - t < timeout:
        # Focus it until its first frames are up (a window not mapped yet ignores it).
        if display and time.time() - t < 6:
            # The window by its pid, else by its title (one app per display).
            w = subprocess.run(["xdotool", "search", "--pid", str(proc.pid), "--name", "noshape"], env=env, capture_output=True, text=True).stdout.split()
            w = w or subprocess.run(["xdotool", "search", "--onlyvisible", "--name", "noshape"], env=env, capture_output=True, text=True).stdout.split()
            if w:
                subprocess.run(["xdotool", "windowfocus", w[-1]], env=env, capture_output=True)
        time.sleep(0.2)
    if proc.poll() is None:
        proc.kill()
        return "timed out"
    err = proc.stderr.read()
    if not out.exists():
        return "no recording: " + " | ".join(l for l in err.splitlines() if "UI-SCRIPT" in l or "panicked" in l)[:300]
    return None


def first_difference(a, b, path=""):
    """The first key path where `a` (live) and `b` (ours) differ, with both values."""
    if isinstance(a, dict) and isinstance(b, dict):
        for k in a:
            if k not in b:
                return (f"{path}.{k}", a[k], "(missing)")
            d = first_difference(a[k], b[k], f"{path}.{k}")
            if d:
                return d
        return None
    if isinstance(a, list) and isinstance(b, list):
        for i, (x, y) in enumerate(zip(a, b)):
            d = first_difference(x, y, f"{path}[{i}]")
            if d:
                return d
        if len(a) != len(b):
            return (f"{path}.length", len(a), len(b))
        return None
    return None if a == b else (path, a, b)


def compare(live, ours):
    """(matched steps, total steps, first difference or None)."""
    pairs = [("start", live.get("start"), ours.get("start"))]
    ours_steps = {s.get("step"): s for s in ours.get("steps", [])}
    for s in live.get("steps", []):
        o = ours_steps.get(s.get("step"), {})
        pairs.append((f"step {s.get('step')}", s.get("state"), o.get("state")))
    matched, first = 0, None
    for name, a, b in pairs:
        a = {k: (a or {}).get(k) for k in KEYS}
        b = {k: (b or {}).get(k) for k in KEYS}
        d = first_difference(a, b)
        if d is None:
            matched += 1
        elif first is None:
            first = (name,) + d
    return matched, len(pairs), first


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("names", nargs="*")
    ap.add_argument("--app", default=os.path.join(os.environ.get("CARGO_TARGET_DIR", str(ROOT / "target")), "debug", "noshape"))
    ap.add_argument("--scenarios", default=str(ROOT / "tests" / "ui-parity"))
    ap.add_argument("--live", default=None, help="live recordings (default: <scenarios>/live)")
    ap.add_argument("--out", default=os.path.join(tempfile.gettempdir(), "noshape-ui-parity"))
    ap.add_argument("--display", default=os.environ.get("DISPLAY", ""))
    ap.add_argument("--timeout", type=float, default=240)
    ap.add_argument("--all", action="store_true", help="every scenario, live recording or not")
    ap.add_argument("--compare-only", action="store_true")
    args = ap.parse_args()
    scen_dir, out_dir = Path(args.scenarios), Path(args.out)
    live_dir = Path(args.live) if args.live else scen_dir / "live"
    out_dir.mkdir(parents=True, exist_ok=True)
    names = args.names or sorted(p.stem for p in scen_dir.glob("*.json") if p.stem != "starts" and (args.all or (live_dir / p.name).exists()))
    if not names:
        print(f"no scenarios with a live recording in {live_dir} (--all runs them anyway)")
        return 1
    score, steps_ok, steps_all = 0, 0, 0
    for name in names:
        out = out_dir / f"{name}.json"
        if not args.compare_only:
            problem = run_one(args.app, scen_dir / f"{name}.json", out, args.display, args.timeout)
            if problem:
                print(f"{name}: {problem}")
                continue
        live = live_dir / f"{name}.json"
        if not live.exists():
            print(f"{name}: recorded ({out}), no live recording")
            continue
        try:
            ours = json.loads(out.read_text())
        except (OSError, ValueError) as e:
            print(f"{name}: {e}")
            continue
        m, n, first = compare(json.loads(live.read_text()), ours)
        steps_ok, steps_all = steps_ok + m, steps_all + n
        if first is None:
            score += 1
            print(f"{name}: match ({n} states)")
        else:
            where, path, a, b = first
            print(f"{name}: {where} differs at {path}: live {json.dumps(a, ensure_ascii=False)} / ours {json.dumps(b, ensure_ascii=False)} ({m}/{n} states)")
    print(f"score: {score}/{len(names)} scenarios, {steps_ok}/{steps_all} states")
    return 0


if __name__ == "__main__":
    sys.exit(main())
