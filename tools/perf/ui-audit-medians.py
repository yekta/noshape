#!/usr/bin/env python3
"""Medians of the UI audit's rounds (tools/perf/ui-audit.sh): one line per timed step, ms. Exits 1 when a step's median
first frame is over 100 ms or the UI thread's CPU to it over 50 ms.

  tools/perf/ui-audit-medians.py <out dir with round*/> <scenario names...>
"""
import glob
import json
import statistics
import sys

FRAME_MS, CPU_MS = 100.0, 50.0


def main():
    out, names = sys.argv[1], sys.argv[2:]
    over = []
    for name in names:
        runs = [json.load(open(f))["steps"] for f in sorted(glob.glob(f"{out}/round*/{name}.json"))]
        print(f"== {name} ({len(runs)} rounds)   handler / UI cpu / first frame (max) / idle")
        for i, step in enumerate(runs[0] if runs else []):
            do = step["do"]
            if do["do"] == "wait":
                continue
            what = do["do"] + " " + json.dumps(do.get("to") if do["do"] == "drag" else do.get("on", do.get("key", do.get("text", ""))))[:60]
            ts = [r[i]["timing"] for r in runs if i < len(r) and r[i].get("timing")]
            failed = [r[i].get("error") for r in runs if i < len(r) and not r[i]["ok"]]
            if failed:
                print(f"{step['step']:3} {what:66} FAILED: {failed[0]}")
                over.append(f"{name} step {step['step']} failed")
                continue
            if not ts:
                continue

            def med(k):
                v = [t[k] for t in ts if t.get(k) is not None]
                return statistics.median(v) if v else float("nan")

            frames = [t["frame"] for t in ts if t.get("frame") is not None]
            print(f"{step['step']:3} {what:66} {med('dispatch'):6.1f} {med('cpu'):6.1f} {med('frame'):6.1f} ({max(frames, default=float('nan')):6.1f}) {med('idle'):6.0f}")
            if med("frame") > FRAME_MS or med("cpu") > CPU_MS:
                over.append(f"{name} step {step['step']} {what}")
    for o in over:
        print("OVER:", o)
    sys.exit(1 if over else 0)


if __name__ == "__main__":
    main()
