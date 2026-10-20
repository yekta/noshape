#!/usr/bin/env python3
"""Summarize viewport benchmark logs (bench.sh / bench.ps1) and apply the gates.

    analyze.py <output dir> <label> [--stills] [--gate-ui-cpu-p95 MS] [--gate-gpu-p95 MS]
               [--gate-p95 MS] [--gate-mode MODE] [--baseline DIR --max-regression FRACTION]

Writes a Markdown table to $GITHUB_STEP_SUMMARY (and stdout) and
<output dir>/summary.json (the next runs' baseline). Gates, on the orbit run of
--gate-mode (default `texture`, else the first mode measured):
  --stills           `texture-still.png` must match `readback-still.png` and not be black;
  --gate-ui-cpu-p95  UI-thread CPU p95 per frame;
  --gate-gpu-p95     our GPU time p95 per frame (skipped for builds that don't report it);
  --gate-p95         frame-interval p95;
  --baseline         every run: fps, frame-interval p95, UI CPU p95 and GPU p95 at most
                     --max-regression worse than in DIR/summary.json.
Exits 1 if a gate fails.
"""
import argparse, glob, json, os, re, sys

SUMMARY = re.compile(
    r"BENCH SUMMARY (?P<kind>\w+) \((?P<path>[^)]*)\): (?P<px>\S+) device px, (?P<inst>\d+) instances \| "
    r"presented (?P<fps>[\d.]+) fps(?:, (?P<renders>\d+) window renders)? \| frame interval mean (?P<mean>[\d.]+) "
    r"p50 (?P<p50>[\d.]+) p95 (?P<p95>[\d.]+) p99 (?P<p99>[\d.]+) max (?P<max>[\d.]+) ms \| over 16.7 ms: (?P<o16>\d+) "
    r"\((?P<o16p>\d+)%\), over 8.3 ms: (?P<o8>\d+) \((?P<o8p>\d+)%\) \| UI-thread CPU per frame mean (?P<cpu>[\d.]+) p95 (?P<cpup95>[\d.]+) ms"
    r"(?: \| GPU per frame mean (?P<gpu>[\d.]+) p95 (?P<gpup95>[\d.]+) ms)?")

# (metric, label, higher is better) compared against the baseline.
COMPARED = [("fps", "fps", True), ("p95", "frame-interval p95", False), ("cpup95", "UI CPU p95", False), ("gpup95", "GPU p95", False)]


def runs(out):
    rows = []
    for log in sorted(glob.glob(os.path.join(out, "*-*.log"))):
        mode = os.path.basename(log).split("-")[0]
        text = open(log, encoding="utf-8", errors="replace").read()
        m = SUMMARY.search(text)
        adapter = re.search(r"GPU: (.*)", text)
        row = {"mode": mode, "log": os.path.basename(log), "adapter": adapter.group(1).strip() if adapter else "?"}
        if m:
            row.update({k: (float(v) if v and re.fullmatch(r"[\d.]+", v) else v) for k, v in m.groupdict().items()})
        else:
            row["kind"] = os.path.basename(log)[len(mode) + 1:-4]
            row["missing"] = True
        rows.append(row)
    return rows


def compare_stills(out):
    """(ok, message) for texture-still.png vs readback-still.png."""
    try:
        from PIL import Image, ImageChops
    except ImportError:
        return False, "Pillow is not installed"
    a_path, b_path = os.path.join(out, "texture-still.png"), os.path.join(out, "readback-still.png")
    if not (os.path.exists(a_path) and os.path.exists(b_path)):
        return False, "missing still screenshots"
    a, b = Image.open(a_path).convert("RGB"), Image.open(b_path).convert("RGB")
    if a.size != b.size:
        return False, f"screenshot sizes differ: {a.size} vs {b.size}"
    w, h = a.size
    # Skip the menu bar / dock bands; the window is centered.
    box = (0, int(h * 0.06), w, int(h * 0.90))
    a, b = a.crop(box), b.crop(box)
    diff = ImageChops.difference(a, b).convert("L").point(lambda v: 255 if v > 16 else 0)
    changed = sum(diff.histogram()[255:]) / (diff.size[0] * diff.size[1])
    center = a.crop((int(w * 0.3), int(h * 0.25), int(w * 0.7), int(h * 0.65))).convert("L")
    hist = center.histogram()
    black = sum(hist[:16]) / (center.size[0] * center.size[1])
    ok = changed < 0.01 and black < 0.25
    return ok, f"texture vs readback: {changed * 100:.2f}% of pixels differ; texture still center {black * 100:.1f}% black"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("out")
    ap.add_argument("label")
    ap.add_argument("--stills", action="store_true")
    ap.add_argument("--gate-p95", type=float)
    ap.add_argument("--gate-ui-cpu-p95", type=float)
    ap.add_argument("--gate-gpu-p95", type=float)
    ap.add_argument("--gate-mode", default="texture")
    ap.add_argument("--baseline")
    ap.add_argument("--max-regression", type=float, default=0.10)
    args = ap.parse_args()
    rows = runs(args.out)
    lines = [f"### Viewport benchmark: {args.label}", "",
             "| run | path | device px | fps | p95 interval | p99 | max | >8.3 ms | >16.7 ms | UI CPU/frame (mean, p95) | GPU/frame (mean, p95) | renders |",
             "|---|---|---|---|---|---|---|---|---|---|---|---|"]
    for r in rows:
        if r.get("missing"):
            lines.append(f"| {r['mode']} {r.get('kind')} | **no BENCH SUMMARY** | | | | | | | | | | |")
            continue
        gpu = f"{r['gpu']:.2f}, {r['gpup95']:.2f} ms" if r.get("gpu") is not None else "n/a"
        renders = int(r["renders"]) if r.get("renders") else ""
        lines.append(f"| {r['mode']} {r['kind']} | {r['path']} | {r['px']} | {r['fps']:.1f} | {r['p95']:.2f} ms | {r['p99']:.2f} | {r['max']:.2f} | "
                     f"{int(r['o8'])} ({int(r['o8p'])}%) | {int(r['o16'])} ({int(r['o16p'])}%) | {r['cpu']:.2f}, {r['cpup95']:.2f} ms | {gpu} | {renders} |")
    if rows:
        lines += ["", f"GPU: {rows[0]['adapter']}"]
    failures = []
    if args.stills:
        ok, msg = compare_stills(args.out)
        lines += ["", f"Zero-copy correctness: {'OK' if ok else 'FAILED'} ({msg})"]
        if not ok:
            failures.append(f"correctness: {msg}")
    measured = [r for r in rows if not r.get("missing")]
    modes = [r["mode"] for r in measured]
    gate_mode = args.gate_mode if args.gate_mode in modes else (modes[0] if modes else None)
    gated = next((r for r in measured if r["mode"] == gate_mode and r.get("kind") == "orbit"), None)

    def gate(name, metric, limit, unit="ms"):
        if limit is None:
            return
        if gated is None:
            failures.append(f"{name}: no orbit result")
            lines.append(f"Gate {name}: FAILED (no orbit result)")
        elif gated.get(metric) is None:
            lines.append(f"Gate {name}: not reported by this build (skipped)")
        else:
            ok = gated[metric] <= limit
            lines.append(f"Gate {name} ({gate_mode} orbit) {gated[metric]:.2f} <= {limit} {unit}: {'OK' if ok else 'FAILED'}")
            if not ok:
                failures.append(f"{name}: {gate_mode} orbit {gated[metric]:.2f} {unit} > {limit} {unit}")

    lines.append("")
    gate("UI-thread CPU p95", "cpup95", args.gate_ui_cpu_p95)
    gate("GPU p95", "gpup95", args.gate_gpu_p95)
    gate("frame-interval p95", "p95", args.gate_p95)
    base_path = os.path.join(args.baseline, "summary.json") if args.baseline else None
    if base_path and os.path.exists(base_path):
        base = json.load(open(base_path, encoding="utf-8"))
        lines += ["", f"Against the baseline ({base.get('label', '?')}), max {args.max_regression * 100:.0f}% worse:"]
        for r in measured:
            b = next((x for x in base.get("runs", []) if x.get("mode") == r["mode"] and x.get("kind") == r.get("kind") and not x.get("missing")), None)
            if not b:
                continue
            notes = []
            for metric, label, higher_better in COMPARED:
                now, was = r.get(metric), b.get(metric)
                if now is None or was is None or was == 0:
                    continue
                change = (now - was) / was
                worse = -change if higher_better else change
                notes.append(f"{label} {was:.2f} → {now:.2f} ({change * 100:+.0f}%)")
                if worse > args.max_regression:
                    failures.append(f"regression: {r['mode']} {r['kind']} {label} {was:.2f} → {now:.2f}")
            lines.append(f"- {r['mode']} {r['kind']}: " + "; ".join(notes))
    elif args.baseline:
        lines += ["", "No baseline (first run on main?)."]
    text = "\n".join(lines) + "\n"
    print(text)
    if os.environ.get("GITHUB_STEP_SUMMARY"):
        with open(os.environ["GITHUB_STEP_SUMMARY"], "a", encoding="utf-8") as f:
            f.write(text)
    with open(os.path.join(args.out, "summary.json"), "w", encoding="utf-8") as f:
        json.dump({"label": args.label, "runs": rows, "failures": failures}, f, indent=1)
    for f in failures:
        print(f"::error::{args.label}: {f}")
    sys.exit(1 if failures else 0)


if __name__ == "__main__":
    main()
