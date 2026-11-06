#!/usr/bin/env python3
"""Coverage of the Part Studio by the interaction suite: how many scenarios exercise each toolbar tool, sketch tool,
context / dropdown menu item and shortcut (tests/ui-parity/README.md "Coverage").

    tools/ui-parity/coverage.py [--app <noshape binary>] [--markdown] [--uncovered]

The universe: the Part Studio toolbar (crates/part-studio-ui `TOOLBAR`, names from the `feature_types` op), the sketch
tools and the Part Studio / camera menu items (crates/ops/src/coverage.rs), the General, Part Studio, 3D view and Sketch
shortcuts (the `ui_shortcuts` op: the Keyboard shortcuts sheet). A feature tool counts for a scenario that clicks it,
presses its shortcut or double-clicks a feature of its type (from its start document or made in the scenario).
"""
import argparse
import json
import os
import re
import subprocess
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
# The order the lead filled gaps in (11-06): the most-used Part Studio tools.
MOST_USED = ["Extrude", "Sketch", "Fillet", "Chamfer", "Revolve", "Shell", "Hole", "Linear pattern", "Circular pattern",
             "Curve pattern", "Mirror", "Boolean", "Plane", "Mate connector"]


def op(app, name):
    out = subprocess.run([app, "call", name, "{}"], capture_output=True, text=True, check=True).stdout
    return json.loads(out)


def toolbar_ids():
    src = (ROOT / "crates/part-studio-ui/src/part_studio/mod.rs").read_text()
    block = src[src.index("const TOOLBAR: &[&[&[&str]]] = &["):]
    block = block[: block.index("\n];")]
    return re.findall(r'"([A-Za-z]+)"', block)


def coverage_rs(prefixes, table):
    src = (ROOT / "crates/ops/src/coverage.rs").read_text()
    block = src[src.index(f"pub const {table}"):]
    block = block[: block.index("\n];")]
    out = []
    for m in re.finditer(r'^\s*\("([^"]+)"', block, re.M):
        area, _, label = m.group(1).partition(":")
        if area in prefixes:
            out.append((area, label))
    return out


def norm_key(k):
    """A key as the scenarios write it: lower case, `mod` for Ctrl/Cmd, `+` between parts."""
    k = k.strip().lower().replace("ctrl/cmd", "mod").replace("cmd", "mod").replace("ctrl", "mod")
    parts = [p for p in re.split(r"[ +]|(?<=[a-z])-(?=.)", k) if p]
    return "+".join(parts)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--app", default="/root/green/noshape")
    ap.add_argument("--scenarios", default=str(ROOT / "tests/ui-parity"))
    ap.add_argument("--markdown", action="store_true", help="the README's coverage table")
    ap.add_argument("--uncovered", action="store_true", help="list what no scenario exercises")
    args = ap.parse_args()
    types = {t["type"]: t for t in op(args.app, "feature_types")["feature_types"]}
    by_name = {t["name"].lower(): t["type"] for t in types.values()}
    shortcuts = op(args.app, "ui_shortcuts")["tabs"]

    items = {}  # (category, name) -> set of scenarios
    tool_of = {}  # a Part Studio toolbar name a scenario clicks -> (category, name)
    sketch_tool_of = {}  # the same in a sketch
    for tid in toolbar_ids():
        name = types.get(tid, {}).get("name", tid)
        items[("toolbar", name)] = set()
        tool_of[name.lower()] = ("toolbar", name)
    for _, label in coverage_rs({"sketch"}, "SHORTCUTS"):
        name = label.rstrip("*")
        items[("sketch tool", name)] = set()
        sketch_tool_of[name.lower()] = ("sketch tool", name)
    # The sketch toolbar's own tooltips where they differ from the shortcut sheet's names.
    for tip, name in (("mirror", "Mirror sketch"), ("use / project edges", "Use"), ("center rectangle", "Center point rectangle"), ("fillet", "Sketch fillet"), ("chamfer", "Sketch chamfer")):
        if ("sketch tool", name) in items:
            sketch_tool_of[tip] = ("sketch tool", name)
    menu_of = {}
    for area, label in coverage_rs({"partStudio", "view"}, "MENUS"):
        items[("menu", label)] = set()
        menu_of[label.lower()] = ("menu", label)
    # A sketch dimension's menu: coverage.rs lists its rows by id.
    for row, item in (("change to driven dimension", "sk-dim-driving"), ("change to driving dimension", "sk-dim-driving"), ("delete dimension", "sk-dim-delete"), ("unconfigure dimension", "sk-dim-unconf")):
        if ("menu", item) in items:
            menu_of[row] = ("menu", item)
    key_of = defaultdict(list)
    for tab in shortcuts:
        if tab["tab"] not in ("General", "Part Studio", "3D view", "Sketch"):
            continue
        for row in tab["rows"]:
            item = ("shortcut", f"{tab['tab']}: {row['name'].rstrip('*')}")
            items[item] = set()
            # A shortcut that starts a tool exercises the tool too (Sketch › Line, `l`).
            tool = row["name"].rstrip("*").lower()
            in_sketch = tab["tab"] == "Sketch"
            also = [(sketch_tool_of if in_sketch else tool_of)[tool]] if tool in (sketch_tool_of if in_sketch else tool_of) else []
            for alt in str(row.get("keys") or "").split(" / "):
                if alt.strip():
                    key_of[norm_key(alt)].extend((it, in_sketch) for it in [item] + also)
    for t in types.values():
        if t.get("shortcut"):
            key_of[norm_key(t["shortcut"])].append((("toolbar", t["name"]), False))

    unmatched = defaultdict(int)
    starts = json.loads((Path(args.scenarios) / "starts.json").read_text())
    for path in sorted(Path(args.scenarios).glob("*.json")):
        if path.stem == "starts":
            continue
        sc = json.loads(path.read_text())
        name = sc.get("name", path.stem)
        start = starts.get(sc.get("start"), {}) if isinstance(sc.get("start"), str) else {}
        feature_type = {f.get("name"): ("sketch" if f.get("type") == "newSketch" else f.get("type")) for f in start.get("features", [])}
        # Whether the step is in a sketch (its toolbar and keys are the sketch's).
        sketching = False
        for step in sc.get("steps", []):
            on = step.get("on") or {}
            if "tool" in on:
                tools = sketch_tool_of if sketching else tool_of
                for label in (on.get("menu"), on["tool"]):
                    if label and label.lower() in tools:
                        items[tools[label.lower()]].add(name)
                        break
                else:
                    unmatched[f"tool {on['tool']}" + (f" › {on['menu']}" if on.get("menu") else "")] += 1
                if on["tool"] == "Sketch":
                    sketching = True
                elif not sketching or on["tool"].lower() in tool_of and on["tool"].lower() not in sketch_tool_of:
                    sketching = False
            if on.get("button") == "ok":
                sketching = False
            if "menu" in on and "tool" not in on:
                if on["menu"].lower() in menu_of:
                    items[menu_of[on["menu"].lower()]].add(name)
                else:
                    unmatched[f"menu {on['menu']}"] += 1
            if step.get("do") == "key":
                key = norm_key(step.get("key", ""))
                for item, sketch_key in key_of.get(key, []):
                    if item in items and sketch_key == sketching:
                        items[item].add(name)
                if key == "shift+s" and not sketching:
                    sketching = True
            if step.get("do") == "dblclick" and "feature" in on:
                row = on["feature"]
                ftype = feature_type.get(row) or by_name.get(re.sub(r"\s+\d+$", "", row).lower())
                tname = types.get(ftype, {}).get("name")
                if ftype == "sketch":
                    sketching = True
                if tname and ("toolbar", tname) in items:
                    items[("toolbar", tname)].add(name)

    cats = ["toolbar", "sketch tool", "menu", "shortcut"]
    if args.markdown:
        print("| kind | covered | scenarios per item (median) |")
        print("|---|---|---|")
    for cat in cats:
        rows = {n: s for (c, n), s in items.items() if c == cat}
        covered = sum(1 for s in rows.values() if s)
        counts = sorted(len(s) for s in rows.values())
        med = counts[len(counts) // 2] if counts else 0
        if args.markdown:
            print(f"| {cat} | {covered}/{len(rows)} | {med} |")
        else:
            print(f"{cat}: {covered}/{len(rows)} covered, median {med} scenarios per item")
    if args.markdown:
        print()
        print("| most-used tool (lead 11-06) | scenarios |")
        print("|---|---|")
        for tool in MOST_USED:
            print(f"| {tool} | {len(items.get(('toolbar', tool), items.get(('sketch tool', tool), set())))} |")
        sketch = [s for (c, _), s in items.items() if c == "sketch tool"]
        print(f"| sketch tools (all {len(sketch)}) | {len(set().union(*sketch))} scenarios, {sum(1 for s in sketch if s)} tools covered |")
    if not args.markdown and unmatched:
        print("\nnot in the universe (a name mismatch, or another studio's):", ", ".join(f"{k} ({v})" for k, v in sorted(unmatched.items(), key=lambda kv: -kv[1])))
    if not args.markdown:
        print()
        for cat in cats:
            for (c, n), s in sorted(items.items(), key=lambda kv: (-len(kv[1]), kv[0][1])):
                if c == cat and (s or args.uncovered):
                    print(f"  {cat:11} {len(s):3}  {n}")


if __name__ == "__main__":
    raise SystemExit(main())
