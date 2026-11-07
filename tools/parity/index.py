#!/usr/bin/env python3
"""The parity index (docs/parity/INDEX.md): one percentage from the suites' own outputs.

    tools/parity/index.py --run <dir> [--green <sha>] [--status-file /tmp/lead/parity-index] [--json <file>]

<dir> holds one suite run's outputs (qa's 4-hourly run writes them there; any missing one leaves its area at 0, and
says so):
    models-report.json   parity_models' report.json (features matching; per document our and live's regeneration)
    assemblies.log       parity_assemblies' output ("assemblies: … occurrences m/n …")
    exchange.log         exchange_round_trip's output ("exchange score: ours imports live's files a/b; live imports ours c/d …")
    uip/score.log        tools/ui-parity/run.py --compare-only (each scenario's "(m/n states)")
    export-compare.txt   tools/drawing-export/compare.py live/ ours/ ("export score: m/n files match")
    open-times.json      tools/perf/open-times.py of the documents live timed (open → model frame)
The coverage of the interaction areas comes from tools/ui-parity/coverage*.py, run here (at this checkout).

Weights are read from INDEX.md's table, so they can't drift from the definition. Interaction areas by scenario name:
asm-* assembly, drw-* drawing, fs-* Feature Studio, vs-* Variable Studio, ver-* versions, doc-* / docs-* / prefs-*
documents, every other recorded scenario the Part Studio's and sketch's; each area's coverage from its coverage script. Speed:
each case scores 1 if ours <= live, else live / ours; a case without a live time is left out.
"""
import argparse
import json
import re
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
LIVE_OPEN = Path("/root/onshape-research/live/open-times.json")


def weights():
    """{area: (weight, has a suite)} from INDEX.md's table"""
    out = {}
    for line in (ROOT / "docs/parity/INDEX.md").read_text().splitlines():
        m = re.match(r"^\| ([^|]+?) \| (\d+) \| ([^|]*) \|", line)
        if m:
            out[m[1]] = (int(m[2]), "no suite yet" not in m[3])
    return out


def read(p):
    return p.read_text(errors="replace") if p.exists() else None


def frac(a, b):
    return a / b if b else None


PREFIXES = (("asm-", "asm"), ("drw-", "drw"), ("doc-", "doc"), ("docs-", "doc"), ("prefs-", "doc"), ("fs-", "fs"),
            ("vs-", "vs"), ("ver-", "ver"))


def states(score_log):
    """matched / recorded states by area, from run.py's per-scenario lines"""
    acc = {k: [0, 0] for k in ("ps", "asm", "drw", "doc", "fs", "vs", "ver")}
    for line in score_log.splitlines():
        m = re.match(r"^([A-Za-z0-9._-]+): (match \((\d+) states\)|.* \((\d+)/(\d+) states\))", line)
        if not m or m[1] == "score":
            continue
        area = next((a for p, a in PREFIXES if m[1].startswith(p)), "ps")
        ok, n = (int(m[3]), int(m[3])) if m[3] else (int(m[4]), int(m[5]))
        acc[area][0] += ok
        acc[area][1] += n
    return acc


def coverage(script):
    """covered / total over every kind the coverage script lists (tools, menus, shortcuts, exports, ...)"""
    try:
        out = subprocess.run([sys.executable, str(ROOT / "tools/ui-parity" / script)], capture_output=True, text=True, timeout=300, cwd=ROOT).stdout
    except subprocess.TimeoutExpired:
        return None, "timed out"
    total = re.search(r"^coverage [^:]+: (\d+)/(\d+) items", out, re.M)
    if total:
        return frac(int(total[1]), int(total[2])), f"{total[1]}/{total[2]}"
    pairs = re.findall(r"^[a-z][a-z /]*: (\d+)/(\d+) covered", out, re.M)
    if not pairs:
        return None, "no totals"
    c, t = sum(int(a) for a, _ in pairs), sum(int(b) for _, b in pairs)
    return frac(c, t), f"{c}/{t}"


def speed_cases(report, open_times):
    cases = []
    if report:
        for m in report.get("models", []):
            r = m.get("regen_ms") or {}
            if r.get("ours") and r.get("live"):
                cases.append(("regen " + m["name"], r["ours"], r["live"]))
    live_open = json.loads(LIVE_OPEN.read_text()) if LIVE_OPEN.exists() else {}
    for name, row in (open_times or {}).items():
        lv = (live_open.get(name) or {}).get("model_ms")
        if row.get("model_ms") and lv:
            cases.append(("open " + name, row["model_ms"], lv))
    return cases


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--run", required=True, type=Path)
    ap.add_argument("--green", default="?")
    ap.add_argument("--status-file")
    ap.add_argument("--json")
    a = ap.parse_args()
    run, table = a.run, weights()
    w = {k: v[0] for k, v in table.items()}
    score, note = {}, {}

    rep = json.loads(read(run / "models-report.json")) if (run / "models-report.json").exists() else None
    if rep:
        score["Part Studio geometry"] = frac(rep["features_matching"], rep["features"])
        note["Part Studio geometry"] = f"features {rep['features_matching']}/{rep['features']}"

    log = read(run / "uip/score.log")
    st = states(log) if log else None
    for area, prefix, script in (("Part Studio and sketch interaction", "ps", "coverage.py"),
                                 ("Assembly interaction", "asm", "coverage_assembly.py"),
                                 ("Drawing interaction", "drw", "coverage_drawing.py"),
                                 ("Feature Studio and FeatureScript editor", "fs", "coverage_feature_studio.py"),
                                 ("Variable Studio", "vs", "coverage_variable_studio.py"),
                                 ("Versions, branches, history, release", "ver", "coverage_versions.py"),
                                 ("Documents, tabs, start page, preferences", "doc", "coverage_documents.py")):
        if not st:
            continue
        if not (ROOT / "tools/ui-parity" / script).exists():
            continue
        m = frac(*st[prefix])
        cov, ctext = coverage(script)
        if m is not None and cov is not None:
            score[area] = m * cov
            note[area] = f"states {st[prefix][0]}/{st[prefix][1]} × coverage {ctext}"
        else:
            note[area] = f"no live recordings yet (coverage {ctext})"

    asm = read(run / "assemblies.log")
    mm = asm and re.search(r"^assemblies: .*occurrences (\d+)/(\d+)", asm, re.M)
    if mm:
        score["Assembly results"] = frac(int(mm[1]), int(mm[2]))
        note["Assembly results"] = f"occurrences {mm[1]}/{mm[2]}"

    exp = read(run / "export-compare.txt")
    mm = exp and re.search(r"^export score: (\d+)/(\d+) files match", exp, re.M)
    if mm:
        score["Drawing export"] = frac(int(mm[1]), int(mm[2]))
        note["Drawing export"] = f"files {mm[1]}/{mm[2]}"

    ex = read(run / "exchange.log")
    mm = ex and re.search(r"exchange score: ours imports live's files (\d+)/(\d+); live imports ours (\d+)/(\d+)", ex)
    if mm:
        halves = [h for h in (frac(int(mm[1]), int(mm[2])), frac(int(mm[3]), int(mm[4]))) if h is not None]
        if halves:
            score["Data exchange"] = sum(halves) / len(halves)
        note["Data exchange"] = f"ours imports {mm[1]}/{mm[2]}, live imports ours {mm[3]}/{mm[4]}"

    ot = json.loads(read(run / "open-times.json")) if (run / "open-times.json").exists() else None
    cases = speed_cases(rep, ot)
    if cases:
        score["Speed against Onshape"] = sum(1.0 if o <= l else l / o for _, o, l in cases) / len(cases)
        nr, no = sum(c[0].startswith("regen") for c in cases), sum(c[0].startswith("open") for c in cases)
        at = sum(o <= l for _, o, l in cases)
        note["Speed against Onshape"] = f"{len(cases)} cases ({nr} regenerations, {no} opens; no live latency times yet), {at} at or under live"

    for area in w:
        if area not in score:
            score[area] = 0.0
            note[area] = note.get(area) or ("not measured in this run" if table[area][1] else "no suite yet")
    index = sum(w[k] * score[k] for k in w) / sum(w.values()) * 100
    stamp = time.strftime("%Y-%m-%d %H:%M", time.gmtime())
    parts = " ".join(f"{re.sub(r'[^A-Za-z]+', '-', k).strip('-').lower()}={score[k] * 100:.1f}" for k in w)
    line = f"{stamp} {a.green} index={index:.1f} {parts}"
    print(line)
    for k in w:
        print(f"  {k} (w {w[k]}): {score[k] * 100:.1f} %  {note.get(k, '')}")
    if a.status_file:
        Path(a.status_file).write_text(line + "\n")
    if a.json:
        Path(a.json).write_text(json.dumps({"time": stamp, "green": a.green, "index": index, "weights": w,
                                            "areas": {k: {"score": score[k], "from": note.get(k, "")} for k in w}}, indent=1))


if __name__ == "__main__":
    main()
