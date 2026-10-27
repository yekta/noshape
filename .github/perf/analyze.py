#!/usr/bin/env python3
"""Summarize viewport benchmark logs (bench.sh / bench.ps1) and apply the gates.

    analyze.py <output dir> <label> [--stills] [--gate-ui-cpu-p95 MS] [--gate-gpu-p95 MS]
               [--gate-p95 MS] [--gate-mode MODE] [--baseline DIR --max-regression FRACTION]
               [--report-regressions]

Writes a Markdown table to $GITHUB_STEP_SUMMARY (and stdout) and
<output dir>/summary.json (the next runs' baseline). Gates, on the orbit run of
--gate-mode (default `texture`, else the first mode measured):
  --stills           `texture-still.png` must match `readback-still.png` and not be black;
  --gate-ui-cpu-p95  UI-thread CPU p95 per frame;
  --gate-gpu-p95     our GPU time p95 per frame (skipped for builds that don't report it);
  --gate-p95         frame-interval p95;
  --baseline         every run: fps, frame-interval p95, UI CPU p95 and GPU p95 at most
                     --max-regression worse than in DIR/summary.json, and worse by more
                     than the metric's noise floor (MIN_CHANGE);
  --report-regressions  compare with the baseline but don't fail on it (software renderers:
                     WARP / lavapipe draw on the runner's shared CPU at 1-2 fps and swing
                     10-50% between runs of the same build).
Exits 1 if a gate fails.
"""
import argparse, glob, json, os, re, sys

SUMMARY = re.compile(
    r"BENCH SUMMARY (?P<kind>\w+) \((?P<path>[^)]*)\): (?P<px>\S+) device px, (?P<inst>\d+) instances \| "
    r"presented (?P<fps>[\d.]+) fps(?:, (?P<renders>\d+) window renders)? \| frame interval mean (?P<mean>[\d.]+) "
    r"p50 (?P<p50>[\d.]+) p95 (?P<p95>[\d.]+) p99 (?P<p99>[\d.]+) max (?P<max>[\d.]+) ms \| over 16.7 ms: (?P<o16>\d+) "
    r"\((?P<o16p>\d+)%\), over 8.3 ms: (?P<o8>\d+) \((?P<o8p>\d+)%\) \| UI-thread CPU per frame mean (?P<cpu>[\d.]+) p95 (?P<cpup95>[\d.]+) ms"
    r"(?: \| window draw per frame mean (?P<draw>[\d.]+) p95 (?P<drawp95>[\d.]+) ms, drawable wait mean (?P<wait>[\d.]+) p95 (?P<waitp95>[\d.]+) ms)?"
    r"(?: \| GPU per frame mean (?P<gpu>[\d.]+) p95 (?P<gpup95>[\d.]+) ms(?P<done> \(submit to done\))?)?")

# (metric, label, higher is better) compared against the baseline.
COMPARED = [("fps", "fps", True), ("p95", "frame-interval p95", False), ("cpup95", "UI CPU p95", False), ("gpup95", "GPU p95", False)]
# Smallest change that counts as a regression, whatever the percentage: below it a runner's jitter decides
# (v0.4.11 macOS 15: UI CPU p95 0.79 -> 0.87 ms failed as +10%).
MIN_CHANGE = {"fps": 2.0, "p95": 2.0, "cpup95": 0.5, "gpup95": 0.5}


# Startup milestones (NOSHAPE_PERF=1 logs), ms from process start; the
# pipelines' compile time in ms.
STARTUP = {
    "window_ms": re.compile(r"first workspace render: ([\d.]+) ms since start"),
    "frame_ms": re.compile(r"first viewport frame presented: ([\d.]+) ms since start"),
    "pipelines_ms": re.compile(r"pipelines for \d+x MSAA built in ([\d.]+) ms"),
    "opening_ms": re.compile(r"opening document: ([\d.]+) ms since start"),
}

# docs/PLAN.md 2.7 speed targets (psui): the frame-driven 2× runs (`retina@orbit` … variants) at 120 fps; the first
# viewport frame within 1 s of process start (the job's later launches; its first pays the OS's first-run costs);
# a document's first frame within 0.5 s of its open.
TARGET_FPS = 120.0
TARGET_FIRST_FRAME_MS = 1000.0
TARGET_OPEN_MS = 500.0


def runs(out):
    rows = []
    for log in sorted(glob.glob(os.path.join(out, "*-*.log"))):
        mode = os.path.basename(log).split("-")[0]
        text = open(log, encoding="utf-8", errors="replace").read()
        m = SUMMARY.search(text)
        adapter = re.search(r"GPU: (.*)", text)
        row = {"mode": mode, "log": os.path.basename(log), "adapter": adapter.group(1).strip() if adapter else "?"}
        for key, pat in STARTUP.items():
            s = pat.search(text)
            if s:
                row[key] = float(s.group(1))
        if m:
            row.update({k: (float(v) if v and re.fullmatch(r"[\d.]+", v) else v) for k, v in m.groupdict().items()})
            # Metal has no timestamps: its GPU time is each frame's submit to done (queueing included). Kept apart
            # from the timestamp GPU time (its gate and baseline), used for the speed targets.
            if row.pop("done", None):
                row["done"], row["donep95"] = row.pop("gpu"), row.pop("gpup95")
                row["gpu"] = row["gpup95"] = None
        else:
            row["kind"] = os.path.basename(log)[len(mode) + 1:-4]
            row["missing"] = True
        rows.append(row)
    return rows


def speed_targets(rows, started):
    """Report lines for the PLAN 2.7 speed targets (report only: not gates until met)."""
    out = ["Speed targets (docs/PLAN.md 2.7):", ""]
    for kind in ("orbit", "pan", "zoom"):
        r = next((x for x in rows if x["mode"] == "texture+retina" and x.get("kind") == kind and not x.get("missing")), None)
        if r is None:
            out.append(f"- {kind} at 2× (frame-driven): not run")
        else:
            # What the frame costs us (the runner's own presentation slows down later in a job): the fps the
            # viewport could sustain, from the slower of the UI thread's and the GPU's p95 per frame.
            gpu = r["gpup95"] if r.get("gpup95") is not None else r.get("donep95")
            cost = max(r["cpup95"], gpu) if gpu is not None else None
            capable = f"; sustainable ≈ {1000.0 / cost:.0f} fps from UI CPU p95 {r['cpup95']:.2f} ms, GPU p95 {gpu:.2f} ms" if cost else ""
            out.append(f"- {kind} at 2× (frame-driven, {r['px']} device px): presented {r['fps']:.1f} fps{capable}; target {TARGET_FPS:.0f}: "
                       f"{'met' if r['fps'] >= TARGET_FPS or (cost is not None and 1000.0 / cost >= TARGET_FPS) else 'not met'}")
    # Cold: the job's first launch (shader and OS caches cold), or every launch of a `cold` run (COLD=1 clears the
    # Metal shader cache before each); warm: the later launches.
    cold_run = bool(os.environ.get("COLD"))
    first = [r["frame_ms"] for r in started if r["log"] == "texture-orbit.log"]
    cold = [r["frame_ms"] for r in started] if cold_run else first
    warm = [] if cold_run else [r["frame_ms"] for r in started if r["log"] != "texture-orbit.log"]
    for name, vals in (("cold start", cold), ("warm start", warm)):
        if vals:
            med = sorted(vals)[len(vals) // 2]
            out.append(f"- {name} to the first viewport frame (median of {len(vals)}): {med:.0f} ms, target {TARGET_FIRST_FRAME_MS:.0f}: "
                       f"{'met' if med <= TARGET_FIRST_FRAME_MS else 'not met'}")
    opens = [r["frame_ms"] - r["opening_ms"] for r in started if r.get("opening_ms") is not None]
    if opens:
        med = sorted(opens)[len(opens) // 2]
        out.append(f"- open a document to its first frame (median of {len(opens)}): {med:.0f} ms, target {TARGET_OPEN_MS:.0f}: "
                   f"{'met' if med <= TARGET_OPEN_MS else 'not met'}")
    return out


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
    # Windows runners print in cp1252, which has no "→" (baseline lines).
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
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
    ap.add_argument("--report-regressions", action="store_true")
    args = ap.parse_args()
    rows = runs(args.out)
    lines = [f"### Viewport benchmark: {args.label}", "",
             "| run | path | device px | fps | p95 interval | p99 | max | >8.3 ms | >16.7 ms | UI CPU/frame (mean, p95) | window draw/frame, drawable wait (mean) | GPU/frame (mean, p95) | renders |",
             "|---|---|---|---|---|---|---|---|---|---|---|---|---|"]
    for r in rows:
        if r.get("missing"):
            lines.append(f"| {r['mode']} {r.get('kind')} | **no BENCH SUMMARY** | | | | | | | | | | | |")
            continue
        gpu = f"{r['gpu']:.2f}, {r['gpup95']:.2f} ms" if r.get("gpu") is not None else "n/a"
        draw = f"{r['draw']:.2f}, {r['wait']:.2f} ms" if r.get("draw") is not None else "n/a"
        renders = int(r["renders"]) if r.get("renders") else ""
        lines.append(f"| {r['mode']} {r['kind']} | {r['path']} | {r['px']} | {r['fps']:.1f} | {r['p95']:.2f} ms | {r['p99']:.2f} | {r['max']:.2f} | "
                     f"{int(r['o8'])} ({int(r['o8p'])}%) | {int(r['o16'])} ({int(r['o16p'])}%) | {r['cpu']:.2f}, {r['cpup95']:.2f} ms | {draw} | {gpu} | {renders} |")
    if rows:
        lines += ["", f"GPU: {rows[0]['adapter']}"]
    # Launch order: texture orbit (the job's first: cold shader caches), then
    # the rest.
    started = sorted((r for r in rows if r.get("frame_ms") is not None), key=lambda r: (r["log"] != "texture-orbit.log", r["log"]))
    if started:
        ms = lambda v: f"{v:.0f} ms" if v is not None else "n/a"
        lines += ["", "Startup (from process start; the first launch compiles every shader):", "",
                  "| launch | window drawn | first viewport frame | shader compile |", "|---|---|---|---|"]
        for i, r in enumerate(started):
            lines.append(f"| {'first' if i == 0 and r['log'] == 'texture-orbit.log' else 'later'}: {r['log'][:-4]} | {ms(r.get('window_ms'))} | {ms(r.get('frame_ms'))} | {ms(r.get('pipelines_ms'))} |")
    lines += [""] + speed_targets(rows, started)
    failures = []
    ran = {r["mode"] for r in rows if not r.get("missing")}
    if args.stills and not {"texture", "readback"} <= ran:
        # perf.yml `modes` with one presentation mode: nothing to compare.
        lines += ["", f"Zero-copy correctness: skipped (only {', '.join(sorted(ran)) or 'no'} runs)"]
    elif args.stills:
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
    # The benchmark document (perf.yml FIXTURE): a baseline of another one
    # (a run dispatched with `fixture`, e.g. live-30's 24 instances against
    # the NIST assembly's 406) compares nothing.
    fixture = os.environ.get("FIXTURE", "")
    base = json.load(open(base_path, encoding="utf-8")) if base_path and os.path.exists(base_path) else None
    if base is not None and base.get("fixture") != fixture:
        lines += ["", f"Baseline ({base.get('label', '?')}) measured {base.get('fixture') or 'an unrecorded document'}, not {fixture or '?'}: not compared."]
    elif base is not None:
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
                if worse <= args.max_regression or abs(now - was) <= MIN_CHANGE[metric]:
                    continue
                if args.report_regressions:
                    notes[-1] += " over, not gated"
                else:
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
        json.dump({"label": args.label, "fixture": fixture, "runs": rows, "failures": failures}, f, indent=1)
    for f in failures:
        print(f"::error::{args.label}: {f}")
    sys.exit(1 if failures else 0)


if __name__ == "__main__":
    main()
