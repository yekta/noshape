#!/usr/bin/env python3
"""Coverage of an area by the interaction suite, against the area's universe (tools/ui-parity/universe/<area>.json):
every tool, menu row, shortcut, dialog and Preferences control live has there, from live's captures (each item's
`source`). Cloud features (`"cloud": true`: sign-in, sharing, comments, App Store, plan upsells) are listed but not
counted.

    tools/ui-parity/coverage_universe.py <area> [--markdown] [--uncovered]

An item counts as covered by a scenario whose steps contain one of its `via` chains in order (other steps may come
between). A chain is a list of step patterns: `{"do": "rclick", "on": {"doc_tab": "*"}}`, `{"on": {"menu":
"Duplicate"}}`, `{"do": "key", "key": "alt+t"}`; a value "*" matches anything, "re:<regex>" a regular expression
(whole value), and an `on` pattern matches a target holding those keys with those values. An item without `via` is
not covered by anything yet.
"""
import argparse
import json
import re
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def value_matches(pat, val):
    if pat == "*":
        return val is not None
    if isinstance(pat, str) and pat.startswith("re:"):
        return isinstance(val, str) and re.fullmatch(pat[3:], val) is not None
    if isinstance(pat, dict):
        return isinstance(val, dict) and all(value_matches(v, val.get(k)) for k, v in pat.items())
    return pat == val


def step_matches(pat, step):
    for k, v in pat.items():
        if k == "key":
            if norm_key(step.get("key", "")) != norm_key(v):
                return False
        elif not value_matches(v, step.get(k)):
            return False
    return True


def norm_key(k):
    return "+".join(sorted(p for p in k.lower().replace("ctrl", "mod").replace("cmd", "mod").split("+") if p))


def chain_in(chain, steps):
    i = 0
    for step in steps:
        if i < len(chain) and step_matches(chain[i], step):
            i += 1
    return i == len(chain)


def steps_of(sc):
    # A drag's `from` is where its press is: it matches `on` patterns too.
    out = []
    for s in sc.get("steps", []):
        if s.get("do") == "drag":
            s = dict(s, on=s.get("from"))
        out.append(s)
    return out


def coverage(area, scenarios_dir):
    universe = json.loads((Path(__file__).parent / "universe" / f"{area}.json").read_text())
    items = [i for i in universe["items"] if not i.get("cloud")]
    scenarios = []
    for path in sorted(Path(scenarios_dir).glob("*.json")):
        if path.stem == "starts":
            continue
        sc = json.loads(path.read_text())
        scenarios.append((sc.get("name", path.stem), steps_of(sc)))
    hits = {}
    for item in items:
        hits[item["id"]] = [n for n, steps in scenarios if any(chain_in(c, steps) for c in item.get("via") or [])]
    return universe, items, hits


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("area", help="documents | versions | variable-studio | feature-studio")
    ap.add_argument("--scenarios", default=str(ROOT / "tests/ui-parity"))
    ap.add_argument("--markdown", action="store_true")
    ap.add_argument("--uncovered", action="store_true", help="list what no scenario exercises, most used first")
    args = ap.parse_args()
    universe, items, hits = coverage(args.area, args.scenarios)
    by_cat = defaultdict(lambda: [0, 0])
    for item in items:
        by_cat[item["category"]][1] += 1
        by_cat[item["category"]][0] += bool(hits[item["id"]])
    covered = sum(c for c, _ in by_cat.values())
    total = sum(t for _, t in by_cat.values())
    cloud = sum(1 for i in universe["items"] if i.get("cloud"))
    scen = {n for h in hits.values() for n in h}
    if args.markdown:
        print(f"| {args.area}: kind | covered |")
        print("|---|---|")
        for cat, (c, t) in sorted(by_cat.items()):
            print(f"| {cat} | {c}/{t} |")
        print(f"| **all** | **{covered}/{total} ({100 * covered / max(total, 1):.1f} %)** |")
    else:
        for cat, (c, t) in sorted(by_cat.items()):
            print(f"{cat}: {c}/{t}")
    print(f"coverage {args.area}: {covered}/{total} items ({100 * covered / max(total, 1):.1f} %) by {len(scen)} scenarios; {cloud} cloud items not counted")
    if args.uncovered:
        for item in sorted(items, key=lambda i: (i.get("freq", 3), i["category"], i["id"])):
            if not hits[item["id"]]:
                print(f"  freq {item.get('freq', 3)}  {item['category']:15} {item['name']}  ({item['where']})")


if __name__ == "__main__":
    raise SystemExit(main())
