#!/usr/bin/env python3
"""Coverage of drawings by the interaction suite (the drw-* scenarios): how many exercise each drawing tool, context
menu item, Drawing shortcut and export format, by area (insert views, section / detail / broken-out, dimensions, notes,
GD&T, tables, sheets, export).

    tools/ui-parity/coverage_drawing.py [--app <noshape binary>] [--markdown] [--uncovered]

The universe: the drawing tools and menu items in crates/ops/src/coverage.rs ("drawing:" rows), the Drawing tab of the
Keyboard shortcuts sheet (the `ui_shortcuts` op) and the export dialog's formats (crates/ops/src/drawing_export.rs).
A tool counts for a scenario that clicks it (or its dropdown item) or presses its shortcut.
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
    ("section / detail / broken-out", r"section|detail|broken|crop|break view"),
    ("insert views", r"insert view|projected view|auxiliary view"),
    ("dimensions", r"dimension"),
    ("GD&T", r"geometric tolerance|datum|surface finish|weld|edge condition|inspection"),
    ("tables", r"table"),
    ("sheets", r"sheet"),
    ("notes", r"note|callout|centermark|centerline|hatch"),
]
# The most-used drawing tools first (gaps filled in this order).
MOST_USED = ["Insert view", "Projected view", "Section view", "Detail view", "Linear dimension", "Diameter dimension",
             "Radial dimension", "Note", "Centermark", "BOM table", "Geometric tolerance", "Datum", "Hole table",
             "Broken-out section", "Auxiliary view", "Hole/thread callout", "Angular dimension", "Ordinate dimension"]


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
    return [m.group(1) for m in re.finditer(r'^\s*\("drawing:([^"]+)"', block, re.M)]


def formats():
    src = (ROOT / "crates/ops/src/drawing_export.rs").read_text()
    block = src[src.index("pub fn label(self)"):]
    block = block[: block.index("\n    }\n")]
    return re.findall(r'=> "([^"]+)"', block)


def norm(s):
    return re.sub(r"[\s-]+", " ", re.sub(r"\s*\([^)]*\)$", "", s.strip().rstrip("*"))).lower()


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
    for name in coverage_rs("SHORTCUTS"):
        if norm(name) not in tool_of:
            items[("tool", name)] = set()
            tool_of[norm(name)] = ("tool", name)
    for name in coverage_rs("MENUS"):
        if norm(name) in tool_of or norm(name) in menu_of:
            continue
        items[("menu", name)] = set()
        menu_of[norm(name)] = ("menu", name)
    for tab in op(args.app, "ui_shortcuts")["tabs"]:
        if tab["tab"] != "Drawing":
            continue
        for row in tab["rows"]:
            item = ("shortcut", row["name"].rstrip("*"))
            items[item] = set()
            also = [tool_of[norm(row["name"])]] if norm(row["name"]) in tool_of else []
            for alt in str(row.get("keys") or "").split(" / "):
                if alt.strip():
                    key_of[norm_key(alt)].extend([item] + also)
    for f in formats():
        items[("export", f)] = set()
    items[("menu", "Export…")] = set()

    unmatched = defaultdict(int)
    scenarios = 0
    for path in sorted(Path(args.scenarios).glob("drw-*.json")):
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
            elif "menu" in on and "dropdown" not in on:
                label = norm(on["menu"].rstrip("…").rstrip("."))
                hit = menu_of.get(norm(on["menu"])) or menu_of.get(label) or menu_of.get(label + "...")
                if hit:
                    items[hit].add(name)
                elif re.match(r"export", on["menu"], re.I):
                    items[("menu", "Export…")].add(name)
                else:
                    unmatched[f"menu {on['menu']}"] += 1
            if step.get("do") == "key":
                for item in key_of.get(norm_key(step.get("key", "")), []):
                    items[item].add(name)
            if ("export", on.get("pick")) in items:
                items[("export", on["pick"])].add(name)

    by_area = defaultdict(lambda: [0, 0])
    for (kind, n), s in items.items():
        a = "export" if kind == "export" else area_of(n)
        by_area[a][1] += 1
        by_area[a][0] += bool(s)
    kinds = ["tool", "menu", "shortcut", "export"]
    if args.markdown:
        print(f"{scenarios} drw-* scenarios\n")
        print("| kind | covered |\n|---|---|")
        for k in kinds:
            rows = [s for (c, _), s in items.items() if c == k]
            print(f"| {k} | {sum(1 for s in rows if s)}/{len(rows)} |")
        print("\n| area | covered |\n|---|---|")
        for a, (c, t) in sorted(by_area.items()):
            print(f"| {a} | {c}/{t} |")
        return
    print(f"{scenarios} drw-* scenarios")
    for k in kinds:
        rows = [s for (c, _), s in items.items() if c == k]
        print(f"{k}: {sum(1 for s in rows if s)}/{len(rows)} covered")
    print("\nby area (tools, menu items, shortcuts and formats together):")
    for a, (c, t) in sorted(by_area.items()):
        print(f"  {a:30} {c}/{t}")
    if unmatched:
        print("\nnot in the universe:", ", ".join(f"{k} ({v})" for k, v in sorted(unmatched.items(), key=lambda kv: -kv[1])))
    print()
    for k in kinds:
        for (c, n), s in sorted(items.items(), key=lambda kv: (-len(kv[1]), kv[0][1])):
            if c == k and (s or args.uncovered):
                print(f"  {k:8} {area_of(n) if k != 'export' else 'export':30} {len(s):3}  {n}")


if __name__ == "__main__":
    raise SystemExit(main())
