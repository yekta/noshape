#!/usr/bin/env python3
"""Coverage of assemblies by the interaction suite (the asm-* scenarios): how many exercise each assembly tool, context
menu item, Assembly shortcut and right-hand panel, by area (mates, mate connectors, insert, patterns, groups,
Replicate, exploded views, named positions, BOM, ...).

    tools/ui-parity/coverage_assembly.py [--app <noshape binary>] [--markdown] [--uncovered]

The universe: the assembly toolbar (noshape_ops::tool_search::ASSEMBLY_TOOLS), the tools and menu items in
crates/ops/src/coverage.rs ("assembly:" rows), the Assembly tab of the Keyboard shortcuts sheet (the `ui_shortcuts`
op) and the right-hand panels. A tool counts for a scenario that clicks it (or its dropdown item) or presses its
shortcut; a menu item for one that clicks it.
"""
import argparse
import json
import re
import subprocess
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

# Areas (lead 11-06), first match wins.
AREAS = [
    ("mate connectors", r"mate connector"),
    ("exploded views", r"explod"),
    ("named positions", r"named position"),
    ("BOM", r"bill of materials|\bbom\b|table"),
    ("patterns", r"pattern|mirror"),
    ("relations", r"relation"),
    ("mates", r"mate|fix|lock"),
    ("groups", r"group"),
    ("Replicate", r"replicate"),
    ("insert / replace", r"insert|replace|standard content|version|update all"),
    ("subassemblies / in context", r"subassembly|in context|context|part studio|linked"),
    ("display", r"hide|show|isolate|transparent|display|section view|explode line|zoom|isometric|view normal"),
]
# The most-used assembly tools first (gaps filled in this order).
MOST_USED = ["Insert", "Fastened mate", "Revolute mate", "Mate connector", "Slider mate", "Cylindrical mate",
             "Group", "Assembly linear pattern", "Assembly circular pattern", "Replicate", "Planar mate",
             "Ball mate", "Pin slot mate", "Parallel mate", "Tangent mate", "Replace instance", "Exploded views",
             "Named positions", "Bill of materials", "Gear relation", "Assembly mirror", "Width mate"]
PANELS = ["Exploded views", "Named positions", "Bill of materials"]
# The right strip's buttons by tooltip ({"button": …}) that open the panels.
PANEL_BUTTONS = {"Exploded views": "Exploded views", "Named positions": "Named positions", "BOM Table": "Bill of materials"}
# Shortcut-sheet commands without a toolbar button of their own, exercised by the tools that run them: "Mate relation"
# (no default key) opens the relation dialog, as the four relation tools do.
SAME_AS = {"Mate relation": ["Gear relation", "Rack and pinion relation", "Screw relation", "Linear relation"]}


def area_of(name):
    for area, pat in AREAS:
        if re.search(pat, name, re.I):
            return area
    return "other"


def op(app, name):
    out = subprocess.run([app, "call", name, "{}"], capture_output=True, text=True, check=True).stdout
    return json.loads(out)


def coverage_rs(table):
    src = (ROOT / "crates/ops/src/coverage.rs").read_text()
    block = src[src.index(f"pub const {table}"):]
    block = block[: block.index("\n];")]
    return [m.group(1) for m in re.finditer(r'^\s*\("assembly:([^"]+)"', block, re.M)]


def toolbar():
    src = (ROOT / "crates/ops/src/tool_search.rs").read_text()
    block = src[src.index("pub const ASSEMBLY_TOOLS"):]
    block = block[: block.index("\n];")]
    return re.findall(r'\("[^"]*", "([^"]+)"', block)


def norm(s):
    return re.sub(r"[\s-]+", " ", re.sub(r"\s*\([^)]*\)$", "", s.strip().rstrip("*").rstrip("…").rstrip("."))).lower()


def norm_key(k):
    k = k.strip().lower().replace("ctrl/cmd", "mod").replace("cmd", "mod").replace("ctrl", "mod")
    return "+".join(p for p in re.split(r"[ +]|(?<=[a-z])-(?=.)", k) if p)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--app", default="/root/green/noshape")
    ap.add_argument("--scenarios", default=str(ROOT / "tests/ui-parity"))
    ap.add_argument("--markdown", action="store_true")
    ap.add_argument("--uncovered", action="store_true", help="list what no scenario exercises")
    args = ap.parse_args()

    items = {}  # (kind, name) -> scenarios
    tool_of, menu_of, key_of = {}, {}, defaultdict(list)
    for name in toolbar() + coverage_rs("SHORTCUTS") + PANELS:
        if norm(name) not in tool_of:
            kind = "panel" if name in PANELS else "tool"
            items[(kind, name)] = set()
            tool_of[norm(name)] = (kind, name)
    for name in coverage_rs("MENUS"):
        if norm(name) in tool_of or norm(name) in menu_of:
            continue
        items[("menu", name)] = set()
        menu_of[norm(name)] = ("menu", name)
    for tab in op(args.app, "ui_shortcuts")["tabs"]:
        if tab["tab"] != "Assembly":
            continue
        for row in tab["rows"]:
            item = ("shortcut", row["name"].rstrip("*"))
            items[item] = set()
            also = [tool_of[norm(row["name"])]] if norm(row["name"]) in tool_of else []
            for alt in str(row.get("keys") or "").split(" / "):
                if alt.strip():
                    key_of[norm_key(alt)].extend([item] + also)

    unmatched = defaultdict(int)
    scenarios = 0
    for path in sorted(Path(args.scenarios).glob("asm-*.json")):
        scenarios += 1
        sc = json.loads(path.read_text())
        name = sc.get("name", path.stem)
        for step in sc.get("steps", []):
            on = step.get("on") or {}
            if "tool" in on:
                for label in (on.get("menu"), on["tool"]):
                    hit = label and (tool_of.get(norm(label)) or menu_of.get(norm(label)))
                    if hit:
                        items[hit].add(name)
                        break
                else:
                    unmatched[f"tool {on['tool']}"] += 1
            elif on.get("button") in PANEL_BUTTONS:
                items[("panel", PANEL_BUTTONS[on["button"]])].add(name)
            elif "menu" in on and "dropdown" not in on:
                hit = menu_of.get(norm(on["menu"])) or tool_of.get(norm(on["menu"]))
                # An item labelled at run time ("Switch to Part Studio 1" for "Switch to Part Studio").
                hit = hit or next((v for k, v in menu_of.items() if norm(on["menu"]).startswith(k + " ")), None)
                if hit:
                    items[hit].add(name)
                else:
                    unmatched[f"menu {on['menu']}"] += 1
            if step.get("do") == "key":
                for item in key_of.get(norm_key(step.get("key", "")), []):
                    items[item].add(name)

    for name, tools in SAME_AS.items():
        hit = tool_of.get(norm(name))
        if hit:
            for t in tools:
                items[hit] |= items.get(tool_of.get(norm(t)), set())

    by_area = defaultdict(lambda: [0, 0])
    for (kind, n), s in items.items():
        a = area_of(n)
        by_area[a][1] += 1
        by_area[a][0] += bool(s)
    kinds = ["tool", "panel", "menu", "shortcut"]
    if args.markdown:
        print(f"{scenarios} asm-* scenarios\n")
        print("| kind | covered |\n|---|---|")
        for k in kinds:
            rows = [s for (c, _), s in items.items() if c == k]
            print(f"| {k} | {sum(1 for s in rows if s)}/{len(rows)} |")
        print("\n| area | covered |\n|---|---|")
        for a, (c, t) in sorted(by_area.items()):
            print(f"| {a} | {c}/{t} |")
        return
    print(f"{scenarios} asm-* scenarios")
    for k in kinds:
        rows = [s for (c, _), s in items.items() if c == k]
        print(f"{k}: {sum(1 for s in rows if s)}/{len(rows)} covered")
    print("\nby area (tools, panels, menu items and shortcuts together):")
    for a, (c, t) in sorted(by_area.items()):
        print(f"  {a:30} {c}/{t}")
    if unmatched:
        print("\nnot in the universe:", ", ".join(f"{k} ({v})" for k, v in sorted(unmatched.items(), key=lambda kv: -kv[1])))
    gaps = [n for n in MOST_USED if not any(s for (c, m), s in items.items() if m == n)]
    print("\nmost-used without a scenario, in order:", ", ".join(gaps) or "none")
    print()
    for k in kinds:
        for (c, n), s in sorted(items.items(), key=lambda kv: (-len(kv[1]), kv[0][1])):
            if c == k and (s or args.uncovered):
                print(f"  {k:8} {area_of(n):30} {len(s):3}  {n}")


if __name__ == "__main__":
    raise SystemExit(main())
