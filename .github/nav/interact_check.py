"""Check an interaction replay trace (interact.py): the Part Studio's
selection and hovered entity after each step, against Onshape's behaviour
(docs/parity/interaction.md rows in brackets). Exits non-zero on a mismatch.
"""

import argparse
import json
import sys


def steps(trace):
    """{step: last highlights state at its end} (None: never traced)."""
    out, state, current = {}, None, None
    with open(trace) as f:
        for line in f:
            try:
                r = json.loads(line)
            except ValueError:
                continue
            if "highlights" in r:
                state = r["highlights"]
            elif r.get("at") == "start":
                current = r["step"]
            elif r.get("at") == "end" and r.get("step") == current:
                out[current] = state
    return out


def kinds(state):
    return [s.split("(")[0] for s in state["selection"]] if state else []


CHECKS = [
    # (step, row, description, predicate on the state)
    ("click-top", "1.1", "a click selects the face", lambda s: kinds(s) == ["Face"]),
    ("click-front-adds", "1.1", "a plain click on another face adds it", lambda s: kinds(s) == ["Face", "Face"]),
    ("click-empty-clears", "1.4", "a click in empty space clears the selection", lambda s: kinds(s) == []),
    ("hover-top", "3.1", "the face under the cursor is hovered", lambda s: s is not None and (s["hover"] or "").startswith("Face")),
    ("hover-leave", "3.6", "leaving the graphics clears the hover", lambda s: s is not None and s["hover"] is None),
    ("select-for-space", "1.5", "(setup) a face selected", lambda s: kinds(s) == ["Face"]),
    ("space-clears", "1.5", "Space clears the selection", lambda s: kinds(s) == []),
    ("crossing-box-plane", "1.8", "a crossing box over the Top plane selects it", lambda s: s is not None and "Plane(Top)" in s["selection"]),
]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--trace", required=True)
    a = ap.parse_args()
    got = steps(a.trace)
    failed = 0
    for step, row, what, ok in CHECKS:
        s = got.get(step)
        good = step in got and ok(s)
        failed += not good
        shown = "missing step" if step not in got else json.dumps({"selection": s["selection"], "hover": s["hover"]} if s else None)
        print(f"{'ok  ' if good else 'FAIL'} {step} [{row}] {what}: {shown}")
    print(f"{len(CHECKS) - failed}/{len(CHECKS)} interaction checks passed")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
