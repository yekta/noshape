#!/usr/bin/env python3
"""The drawing completeness matrix (docs/parity/drawing-matrix.md): every tool, dropdown row, right-click menu row and
panel or dialog text of live's drawing, marked in ours as matching, differing, present (not compared) or missing.

    tools/ui-parity/drawing_matrix.py --run <a suite run's uip dir> [--dom <live dom dir>] [--details]

Live's side: the toolbar dump (dom/toolbar-drawing*.txt and its ▾ rows), the right-click menu dumps
(dom/context-menus-drawing*.txt) and the panel and dialog dumps of SURFACES; plus what live's recordings
(tests/ui-parity/live/drw-*.json) show: each state's tool, panel texts and the menu rows the steps click.
Ours, per item:
- matching: a live state shows it (the active tool, a panel text, a clicked menu row) and ours' state of that step shows
  it the same (the tool active, the text in the same panel; a clicked row: the step's state matches live's);
- differs: a live state shows it and ours' state of that step doesn't (and no other state matches it);
- present: no recording shows it, its label is in our sources (crates/*/src);
- missing: neither.
"""
import argparse
import glob
import json
import re
import sys
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(Path(__file__).resolve().parent))
import run as uip  # noqa: E402

# (surface, dump globs, live panel titles); the toolbar and the menus are their own surfaces.
SURFACES = [
    ("Insert view", ["drawing-insert-view-panel.txt"], ["Insert view"]),
    ("Auxiliary view", [], ["Auxiliary view"]),
    ("Section view", ["drawing-section-view-dialog.txt", "drawing-edit-section.txt", "drawing-section-of-section.txt"], ["Section view"]),
    ("Aligned section view", [], ["Aligned section view"]),
    ("Broken-out section", [], ["Broken-out section"]),
    ("Detail view", ["drawing-detail-dialog.txt", "drawing-detail-prompt-*.html"], ["Detail view"]),
    ("Break view", [], ["Break view"]),
    ("Crop view", [], ["Crop view"]),
    ("Dimension", ["drawing-dimension-edit-dialog.txt", "drawing-sketch-dim-editbox.json"], ["(in)", "(mm)"]),
    ("Ordinate dimension", ["drawing-ordinate-rclick.html"], []),
    ("Hole/thread callout", ["drawing-hole-callout-*"], []),
    ("Datum / Datum target", ["drawing-datum-dialog.txt", "drawing-datum-target-*"], ["Datum"]),
    ("Geometric tolerance", ["drawing-gdt-*"], ["Geometric tolerance"]),
    ("Surface finish symbol", [], ["Surface finish"]),
    ("Weld symbol", ["drawing-weld-*"], ["Weld symbol"]),
    ("Edge condition symbol", [], ["Edge condition"]),
    ("Note", ["drawing-note-*"], ["Note"]),
    ("Callout", ["drawing-callout-dialog.txt", "drawing-add-stacked-callouts-dialog.txt"], ["Callout", "Stacked callouts"]),
    ("Table", ["drawing-table-*"], ["Table", "Table properties", "Cell"]),
    ("BOM table", [], ["BOM table", "BOM table properties"]),
    ("Revision table", [], ["Revision table"]),
    ("Cut list table", [], ["Cut list table", "Cut list table properties"]),
    ("Hole table", ["drawing-hole-table-dialog.html", "drawing-hole-cell-dblclick.html", "drawing-holetable-titleblock.json"], ["Hole table"]),
    ("Custom table", ["drawing-custom-table-*"], ["Custom table", "Custom table properties"]),
    ("Centermark", ["drawing-centermark-dialog.txt"], ["Centermark"]),
    ("Hatch region", ["drawing-hatch-*"], ["Hatch region", "Edit hatch"]),
    ("Insert DXF or DWG", ["sk-tool-insert-dxf-dwg.txt"], []),
    ("Insert image", [], ["Image properties", "Resize"]),
    ("Format painter", [], ["Format painter"]),
    ("Inspection item", ["drawing-panel-inspection-table*.txt"], ["Inspection item", "Inspection table", "Reorder inspection items"]),
    ("Adjust linestyle", ["drawing-adjust-linestyle.txt", "drawing-linestyle-colorpicker.txt"], ["Adjust linestyle"]),
    ("Move to sheet / Replicate", ["drawing-move-to-sheet.txt", "drawing-replicate-*"], []),
    ("View properties", ["drawing-view-properties-multi.html", "drawing-flat-view-properties.txt"], ["View properties"]),
    ("Sheets panel", ["drawing-sheets-panel-3sheets.json", "drawing-sheet-row-menu-3sheets.html"], ["Views"]),
    ("Sheet properties", ["drawing-sheet-properties*.txt", "drawing-sheet-properties*.html", "drawing-*-sheet-properties.html"], ["Sheet properties"]),
    ("Drawing properties", ["drawing-properties-*.txt"], ["Drawing properties"]),
    ("Styles panel", ["drawing-panel-styles.txt", "drawing-styles-*"], []),
    ("MBD status", ["drawing-panel-mbd-status.txt"], []),
    ("Reference manager", ["drawing-ref-manager-selective.txt"], ["Reference manager"]),
    ("Title block", ["drawing-titleblock-*"], []),
    ("Shortcut toolbar (s)", ["drawing-shortcut-toolbar.json"], []),
    ("Create Drawing", ["dialog-create-drawing.txt", "drawing-custom-template-*"], ["Create Drawing: New drawing name"]),
    ("Export Drawing", ["dialog-export-drawing*.txt"], ["Export Drawing"]),
    ("Change to version", ["drawing-change-to-version.txt"], []),
]
CLOUD = {"Add comment"}
ACCEPTED = {
    "Formulas": "live 1735: no Formulas in the hole table's Cell panel or menus",
    "All views in a hole table must be of a common singular part": "live 1744: other parts' views aren't pickable, no error",
    "All instances in all views": "live 1735: no Apply to with a single-part section",
}
# Texts in the dumps that are user content, the recording's document, units or the dump format, not UI.
NOISE = re.compile(r"^(Part \d|Part Studio \d|Sheet\d|Drawing \d|Assembly \d|Front|Top|Right|Left|Back|Bottom|Isometric|"
                   r"Hole \d|Extrude \d|Sketch \d|Hatch \d|[A-Z]\d?|[A-Z]-[A-Z]|DETAIL [A-Z]|SECTION [A-Z]-[A-Z]|"
                   r"in|mm|deg|°|\d.*|o\w{6,}|y\d+|x\d+)$")


# Live's account's documents, tabs and the runners' copies: user content, not UI.
USER = re.compile(r"live2?\b|noshape ui parity|^start |Copy \d|\buip\b|^(kern|fs|asb|jt|ctx|pvz|qa\d?|dx|psb|g\d+|w\d+|skb|ui\d?)\s|fluxpad|"
                  r"Sheet\d|Trial - |Templates$|\.dwt$|^Part \d|Drawing \d+$|Load basic HTML|XeTiming|document\.|^\W*$|"
                  r"^(nist|ktane|psas)_|Shaft_With|OsSvgIconId|gathering statistics|[∅↧⌴]|"
                  r"^(active toolbar tool|hints/toasts|graphics overlays):|^button\.")


def norm(s):
    s = s.replace("&amp;nbsp;", " ").replace("&nbsp;", " ").replace(" ", " ").replace("...", "…").strip()
    s = re.sub(r"^\[dis\]|^\*", "", s).strip()
    return re.sub(r"\s+", " ", s)


def key(s):
    return re.sub(r"\s*\((?:[a-z]|shift\+[a-z]|ctrl\+[a-z])\)$", "", norm(s).rstrip("…").strip()).lower()


def texts_of(path):
    """Every visible text, option, tooltip and placeholder of a dump (dx's audit reader)."""
    raw = open(path, errors="ignore").read()
    out = []
    if path.endswith(".json"):
        def walk(v):
            if isinstance(v, str):
                v = re.sub(r"^o\w+@\d+:", "", v)
                if "<" in v:
                    v = re.sub(r"<[^>]*>", " ", v)
                v = re.sub(r"^\w+ComboBox:", "", v)
                out.extend(re.split(r"\s\|\s|\s/\s|,(?=\S)", v) if ("|" in v or "," in v or " / " in v) else [v])
            elif isinstance(v, list):
                for x in v:
                    walk(x)
            elif isinstance(v, dict):
                for x in v.values():
                    walk(x)
        try:
            walk(json.loads(raw))
        except ValueError:
            pass
    else:
        for m in re.finditer(r'(?:\] "|ph="|tip="|title="|>)([^"<>]{2,80})(?:"|<)', raw):
            out.append(m.group(1))
        for m in re.finditer(r"\{([^{}]+)\}", raw):
            out.extend(m.group(1).split(" | "))
        for m in re.finditer(r'= "([^"]+)"', raw):
            out.append(m.group(1))
        if not re.search(r'\] "', raw):
            for line in raw.splitlines():
                t = line.strip()
                if t and not t.startswith(("SHOT", "[", "div", "span", "<", "===", "Wt-popup")) and len(t) < 70:
                    out.append(t)
    res = []
    for t in out:
        t = norm(t)
        if len(t) < 2 or not re.search("[A-Za-z]", t) or t.startswith(("rgb", "http", "#", "data:", "pad=", "/")) or NOISE.match(t):
            continue
        if re.search(r"[{};=\"'›@]|\.svg|\.dxf|\.dwg|\bpx\b|xenon|Wt-|toolbar-|---|OVERLAY|ROOT |DIV\.|TEXTAREA|rgba|^[a-z]+[A-Z]\w+$|\sy\d+$", t):
            continue
        if USER.search(t):
            continue
        if t not in res:
            res.append(t)
    return res


def sources():
    text = []
    for p in glob.glob(str(ROOT / "crates/*/src/**/*.rs"), recursive=True):
        text.append(open(p, errors="ignore").read())
    low = "\n".join(text).lower().replace("\\u{2026}", "…").replace("\\u{b0}", "°")
    return low


def in_sources(src, t):
    low = t.lower()
    if low in src or low.replace("…", "...") in src or low.replace("…", "\\u{2026}") in src:
        return True
    # A text built at run time: its fixed head before a format placeholder ("Delete hatch {}", "Count type: {}").
    words = low.split()
    for n in range(len(words) - 1, 0, -1):
        head = " ".join(words[:n])
        if f'"{head} {{' in src or f'"{head}{{' in src:
            return True
    return False


def toolbar(dom):
    """[(tool, ▾ parent or None)] in live's order: the newest toolbar dump, Edge condition symbol after Weld symbol
    (live 11-01, behaviours "Toolbar map") when the dump predates it."""
    files = sorted(glob.glob(f"{dom}/toolbar-drawing*.txt"), key=lambda p: ("pro" not in p, p))
    raw = open(files[0]).read()
    tools, drops = [], {}
    for m in re.finditer(r'^\S+\.svg @\d+ w\d+ \[[^\]]*\] tip="([^"/]+?) /', raw, re.M):
        tools.append((m.group(1).strip(), None))
    for m in re.finditer(r"^caret@\d+: \[[^\]]*\] (.+)$", raw, re.M):
        rows = [r.strip() for r in m.group(1).split(" | ")]
        drops[rows[0]] = rows[1:]
    if not any(t == "Edge condition symbol" for t, _ in tools):
        at = next(i for i, (t, _) in enumerate(tools) if t == "Weld symbol") + 1
        tools.insert(at, ("Edge condition symbol", None))
    out = []
    for t, _ in tools:
        out.append((t, None))
        out.extend((r, t) for r in drops.get(t, []))
    return out


def menus(dom):
    """{entity: [rows]} of live's right-click menu dumps."""
    out = {}
    for path in sorted(glob.glob(f"{dom}/context-menus-drawing*.txt")):
        name = None
        for line in open(path):
            line = line.rstrip()
            if line.startswith("=== "):
                name = re.sub(r"^rmb-dw-", "", line[4:].strip())
                out.setdefault(name, [])
            elif name and line.startswith("  ") and not line.strip().startswith("—"):
                out[name].append(norm(line.strip()))
    return {k: v for k, v in out.items() if v}


def state_matches(live, ours, whole):
    """Per live step number: whether ours' state of that step matched live's (run.py's comparison, step by step)."""
    steps = live.get("steps", [])
    res, prev = {}, None
    for k in range(len(steps) + 1):
        trunc = dict(live, steps=steps[:k])
        m, _, _ = uip.compare(json.loads(json.dumps(trunc)), json.loads(json.dumps(ours)), whole)
        if k:
            res[steps[k - 1].get("step")] = prev is not None and m > prev
        prev = m
    return res


def recordings(run_dir):
    """Yield (scenario, live step, ours step or None, matched) for every drw-* recording run."""
    live_dir = ROOT / "tests/ui-parity/live"
    scen_dir = ROOT / "tests/ui-parity"
    uip.STARTS = json.loads((scen_dir / "starts.json").read_text())
    for path in sorted(live_dir.glob("drw-*.json")):
        out = Path(run_dir) / path.name
        if not out.exists():
            continue
        live = uip.forget_old_query_states(json.loads(path.read_text()))
        ours = json.loads(out.read_text())
        start = json.loads((scen_dir / path.name).read_text()).get("start") if (scen_dir / path.name).exists() else None
        whole = isinstance(start, str) and any(k in uip.STARTS.get(start, {}) for k in ("tabs", "page"))
        ok = state_matches(live, ours, whole)
        ours_steps = {s.get("step"): s for s in ours.get("steps", [])}
        for s in live.get("steps", []):
            yield path.stem, s, ours_steps.get(s.get("step")), ok.get(s.get("step"), False)


def dialog_texts(state):
    d = (state or {}).get("dialog")
    if not isinstance(d, dict):
        return None, set()
    # A modal's text is the whole body read as one string: its fields' labels stand for it.
    text = d.get("text") or []
    texts = {norm(t) for t in ([] if isinstance(text, str) else text) if isinstance(t, str)}
    for f in d.get("fields") or []:
        if isinstance(f, dict):
            if isinstance(f.get("label"), str):
                texts.add(norm(f["label"]))
            for o in f.get("options") or []:
                if isinstance(o, str):
                    texts.add(norm(o))
    return d.get("title"), texts


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--run", required=True, help="a suite run's uip directory (ours' drw-*.json)")
    ap.add_argument("--dom", default="/root/onshape-research/live/dom")
    ap.add_argument("--details", action="store_true", help="list every item, not only the ones that aren't matching")
    args = ap.parse_args()
    src = sources()

    # What live's recordings show, and how ours compared.
    tool_seen = defaultdict(lambda: [0, 0])  # tool -> [states ours matched (tool active alike), states not]
    text_seen = defaultdict(lambda: [0, 0])  # (panel title, text) -> [ours shows it, ours doesn't]
    menu_seen = defaultdict(lambda: [0, 0])  # menu row -> [step state matched, not]
    scen_of = defaultdict(set)
    for name, ls, os_, ok in recordings(args.run):
        st, ost = ls.get("state") or {}, (os_ or {}).get("state") or {}
        tool = (st.get("drawing") or {}).get("tool")
        if tool:
            alike = (ost.get("drawing") or {}).get("tool") == tool
            tool_seen[key(tool)][0 if alike else 1] += 1
            scen_of[("tool", key(tool))].add(name)
        title, texts = dialog_texts(st)
        if title:
            otitle, otexts = dialog_texts(ost)
            for t in texts:
                text_seen[(title, t)][0 if ok or (otitle == title and t in otexts) else 1] += 1
        on = (ls.get("do") or {}).get("on") or {}
        if isinstance(on, dict) and isinstance(on.get("menu"), str) and "dropdown" not in on:
            menu_seen[key(on["menu"])][0 if ok else 1] += 1
            scen_of[("menu", key(on["menu"]))].add(name)
        if isinstance(on, dict) and isinstance(on.get("tool"), str):
            for label in (on.get("menu"), on["tool"]):
                if isinstance(label, str):
                    scen_of[("tool", key(label))].add(name)

    def status(seen, present):
        if seen and seen[0]:
            return "="
        if seen and seen[1]:
            return "≠"
        return "present" if present else "missing"

    rows = []  # (surface, item, status, note)
    for tool, parent in toolbar(args.dom):
        k = key(tool)
        present = in_sources(src, norm(tool)) or in_sources(src, re.sub(r"\s*\([^)]*\)$", "", tool))
        seen = tool_seen.get(k) or menu_seen.get(k)
        rows.append(("Toolbar" if parent is None else f"Toolbar ▾ {key(parent).capitalize()}", tool, status(seen, present), ", ".join(sorted(scen_of.get(("tool", k), ()))[:2])))
    # Pro-plan tools live's recordings show that the toolbar dump lacks (Revision table: a Free plan dump).
    listed = {key(t) for t, _ in toolbar(args.dom)}
    for k, seen in sorted(tool_seen.items()):
        if k not in listed and not k.endswith(" dimension") and k not in ("dimension", "note", "spline point", "center point circle"):
            rows.append(("Toolbar", k.capitalize(), status(seen, True), "recorded; not in the toolbar dump (Pro plan)"))
    for entity, items in menus(args.dom).items():
        for it in items:
            k = key(it)
            rows.append((f"Menu: {entity}", it, status(menu_seen.get(k), in_sources(src, it) or in_sources(src, it.rstrip("…."))), ""))
    for surface, globs_, titles in SURFACES:
        texts = []
        for g in globs_:
            for p in sorted(glob.glob(f"{args.dom}/{g}")):
                texts += [t for t in texts_of(p) if t not in texts]
        recorded = {t for (title, t) in text_seen if title in titles}
        for t in sorted(recorded):
            if NOISE.match(t) or USER.search(t) or len(t) < 2:
                continue
            seen = [sum(text_seen[(ti, t)][i] for ti in titles if (ti, t) in text_seen) for i in (0, 1)]
            rows.append((surface, t, status(seen, in_sources(src, t)), "recorded"))
        for t in texts:
            if t in recorded:
                continue
            rows.append((surface, t, status(None, in_sources(src, t)), "dump"))

    # Cloud-only rows (out of scope) and dump texts live itself no longer shows (docs/parity/audit-drawing-dataexchange.md).
    rows = [(sf, it, "n/a" if it in CLOUD else ("accepted" if it in ACCEPTED and st != "=" else st), ACCEPTED.get(it, note) if st != "=" else note)
            for sf, it, st, note in rows]
    kinds = ["=", "≠", "present", "missing", "accepted", "n/a"]
    counts = defaultdict(lambda: defaultdict(int))
    for sf, _, st, _ in rows:
        group = "Toolbar" if sf.startswith("Toolbar") else "Right-click menus" if sf.startswith("Menu:") else "Panels and dialogs"
        counts[group][st] += 1
        counts["All"][st] += 1
    print("| surface | items | = matching | ≠ differs | present, not compared | missing | accepted (live has none) | n/a (cloud) |")
    print("|---|---|---|---|---|---|---|---|")
    for g in ("Toolbar", "Right-click menus", "Panels and dialogs", "All"):
        c = counts[g]
        print(f"| {g} | {sum(c.values())} | " + " | ".join(str(c[k]) for k in kinds) + " |")
    print("\n### Toolbar and its ▾ rows\n\n| # | tool | in ▾ of | status | takes |\n|---|---|---|---|---|")
    n = 0
    for sf, it, st, note in rows:
        if sf.startswith("Toolbar"):
            n += 1
            print(f"| {n} | {it} | {sf.removeprefix('Toolbar').removeprefix(' ▾ ')} | {st} | {note} |")
    print("\n### Right-click menus\n\n| entity | row | status |\n|---|---|---|")
    for sf, it, st, _ in rows:
        if sf.startswith("Menu:"):
            print(f"| {sf.removeprefix('Menu: ')} | {it} | {st} |")
    print("\n### Panels and dialogs\n")
    print("| surface | items | = | ≠ | present, not compared | missing | accepted | ≠, missing and accepted items | not compared (first 10) |")
    print("|---|---|---|---|---|---|---|---|---|")
    by_surface = defaultdict(list)
    for r in rows:
        if not r[0].startswith(("Toolbar", "Menu:")):
            by_surface[r[0]].append(r)
    for sf, rs in by_surface.items():
        c = defaultdict(int)
        for r in rs:
            c[r[2]] += 1
        odd = "; ".join(f"{r[1]} ({r[2]}{': ' + r[3] if r[2] == 'accepted' else ''})" for r in rs if r[2] in ("≠", "missing", "accepted"))
        todo = [r[1] for r in rs if r[2] == "present"]
        more = f" … +{len(todo) - 10}" if len(todo) > 10 else ""
        print(f"| {sf} | {len(rs)} | {c['=']} | {c['≠']} | {c['present']} | {c['missing']} | {c['accepted']} | {odd} | {'; '.join(todo[:10])}{more} |")
    if args.details:
        print("\n### Every item\n\n| surface | item | status | source |\n|---|---|---|---|")
        for sf, it, st, note in rows:
            print(f"| {sf} | {it} | {st} | {note} |")

if __name__ == "__main__":
    raise SystemExit(main())
