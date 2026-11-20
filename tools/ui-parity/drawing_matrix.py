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
# Our panels' titles where they aren't live's (a surface's tabs, a palette): what ours shows under them counts for it.
OURS_TITLES = {
    "Drawing properties": ["Annotations", "Dimensions", "Tables", "Views"],
    "Title block": ["Note"],
    "Hole/thread callout": ["Dimension palette"],
    "Ordinate dimension": ["Dimension palette"],
    "Move to sheet / Replicate": ["Move to sheet", "Replicate region"],
    "Styles panel": ["Styles"],
    # A cell's text panel is titled Note (live dom drawing-table-after-ok).
    "Table": ["Table properties", "Note"],
    # drawing-holetable-titleblock.json's tbDbl is the title block's Note panel.
    "Hole table": ["Cell", "Note"],
    "MBD status": ["MBD status"],
    "Change to version": ["Reference manager"],
}


MENUS = "(our runs' menus)"


def ours_shown(run_dir):
    """Every text our build showed in a panel (its fields, an open list or menu, the tooltip), by the panel's title."""
    pool = defaultdict(set)

    def walk(v, out):
        if isinstance(v, str):
            out.add(norm(v))
        elif isinstance(v, list):
            for x in v:
                walk(x, out)
        elif isinstance(v, dict):
            for x in v.values():
                walk(x, out)
    for p in glob.glob(f"{run_dir}/*.json"):
        try:
            steps = json.loads(Path(p).read_text()).get("steps", [])
        except (ValueError, OSError):
            continue
        for s in steps:
            st = s.get("state") or {}
            # A right-click menu our build opened (a sheet row's Move up): counted as our menus are.
            lst = st.get("list")
            if isinstance(lst, dict) and lst.get("kind") == "context-menu":
                walk(lst.get("rows"), pool[MENUS])
            d = st.get("dialog")
            if isinstance(d, dict) and isinstance(d.get("title"), str):
                for part in (d, st.get("list"), st.get("tooltip")):
                    walk(part, pool[d["title"]])
    return pool
ACCEPTED = {
    "Formulas": "live 1735: no Formulas in the hole table's Cell panel or menus",
    "All views in a hole table must be of a common singular part": "live 1744: other parts' views aren't pickable, no error",
    "All instances in all views": "live 1735: no Apply to with a single-part section",
}
# Menu rows that depend on the spot clicked: live's later takes show ours (entity, row as listed).
ACCEPTED_MENU = {
    ("surface-finish-on-edge", "Add leader"): "live2 drw-surface-finish-flip: Flip, no Add leader, on an edge without a leader",
    ("surface-finish-on-edge", "Flip (ours only)"): "live2 drw-surface-finish-flip: Flip on an edge without a leader",
    ("ordinate", "Reset ordinate (ours only)"): "live2 drw-ordinate-value-rclick: on a value's text",
    ("table-cell (click the table, click the cell again, then right-click)", "Merge ▸ All | Horizontal | Vertical"):
        "live2 drw-table-cells-resize s9: Merge greyed for one cell; its rows on a block (drw-table-cells-merge s11)",
}
# Texts in the dumps that are user content, the recording's document, units or the dump format, not UI.
NOISE = re.compile(r"^(Part \d|Part Studio \d|Sheet\d|Drawing \d|Assembly \d|Front|Top|Right|Left|Back|Bottom|Isometric|"
                   r"Hole \d|Extrude \d|Sketch \d|Hatch \d|[A-Z]\d?|[A-Z]-[A-Z]|DETAIL [A-Z]|SECTION [A-Z]-[A-Z]|"
                   r"in|mm|deg|°|\d.*|o\w{6,}|y\d+|x\d+)$")


# Live's account's documents, tabs and the runners' copies: user content, not UI.
USER = re.compile(r"live2?\b|noshape ui parity|^noshape mm test$|^FS sizes$|\bof asb drawing src\b|^start |Copy \d|\buip\b|^(kern|fs|asb|jt|ctx|pvz|qa\d?|dx|psb|g\d+|w\d+|skb|ui\d?)\s|fluxpad|"
                  r"Sheet\d|Trial - |Templates$|\.dwt$|^Part \d|Drawing \d+$|Load basic HTML|XeTiming|document\.|^\W*$|"
                  r"^(nist|ktane|psas)_|Shaft_With|OsSvgIconId|gathering statistics|[∅↧⌴]|"
                  r"^(active toolbar tool|hints/toasts|graphics overlays):|^button\.")


def norm(s):
    s = s.replace("&amp;nbsp;", " ").replace("&nbsp;", " ").replace(" ", " ").replace("...", "…").strip()
    s = re.sub(r"^\[dis\]|^\*", "", s).strip()
    return re.sub(r"\s+", " ", s)


def menu_row_in_sources(src, row):
    """A dump's menu row in our sources; a submenu's ("Merge ▸ All | Horizontal | Vertical") when its name and each of
    its rows are (a "(…)" note in place of rows: the name alone)."""
    if "▸" not in row:
        return in_sources(src, row) or in_sources(src, row.rstrip("…."))
    name, _, subs = row.partition("▸")
    subs = [x.strip() for x in subs.split("|") if x.strip() and not x.strip().startswith("(")]
    return in_sources(src, name.strip()) and all(in_sources(src, x) for x in subs)


def key(s):
    return re.sub(r"\s*\((?:[a-z]|shift\+[a-z]|ctrl\+[a-z])\)$", "", norm(s).rstrip("…").strip()).lower()


def menu_key(row):
    """A menu row's key: a submenu with its rows ("Merge ▸ All | Horizontal | Vertical"), or its name alone where the
    dump notes "(…)" in place of rows; a row naming a tab or document ("Switch to <tab>") by its words before the name."""
    row = row.replace("\tbeta", "")
    name, arrow, subs = norm(row).partition("▸")
    if arrow and (not subs.strip() or subs.strip().startswith("(")):
        row = name
    k = key(re.sub(r"\s+", " ", row.replace("▸", " ▸ ")))
    return re.sub(r"^(switch to|insert custom table of) .*", r"\1 …", k)


def menu_keys(row):
    """Every key a menu row matches: its own and its name's (live's dumps list a submenu by its name)."""
    return {menu_key(row), menu_key(row.partition("▸")[0])}


VOID = {"area", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source", "track", "wbr"}


def strip_hidden(raw):
    """An HTML dump without its display: none elements."""
    from html.parser import HTMLParser
    starts = [0]
    for line in raw.splitlines(keepends=True):
        starts.append(starts[-1] + len(line))
    cuts = []

    class P(HTMLParser):
        def __init__(self):
            super().__init__(convert_charrefs=False)
            self.depth, self.hidden_at, self.hidden_from = 0, None, 0

        def at(self):
            line, col = self.getpos()
            return starts[line - 1] + col

        def handle_starttag(self, tag, attrs):
            if tag in VOID:
                return
            self.depth += 1
            style = (dict(attrs).get("style") or "").replace(" ", "")
            if self.hidden_at is None and "display:none" in style:
                self.hidden_at, self.hidden_from = self.depth, self.at()

        def handle_endtag(self, tag):
            if tag in VOID:
                return
            if self.hidden_at == self.depth:
                cuts.append((self.hidden_from, self.at()))
                self.hidden_at = None
            self.depth -= 1

    P().feed(raw)
    for a, b in reversed(cuts):
        raw = raw[:a] + raw[b:]
    return raw


# Surfaces whose dumps' hidden elements live never shows (live2 11-16, holes-plate: a hole table has no Title row,
# Reorder items or Collapse tags row in its dialog, no insert / remove / merge rows in its Cell popup).
HIDDEN_IS_ABSENT = {"Hole table", "View properties"}
# Texts live never lets a user reach (live2 11-17): display:none in every drawing panel dump (Reorder items, Cut from
# section), or options of the note editor's Flag notes lists, which are disabled in every note and table cell; the
# inspection table's 9th column of the 10-18 dumps (live 11-17: 8 columns).
NEVER_SHOWN = {"Reorder items", "Cut from section", "Double circle", "Square circle", "Tight fit", "Nominal & Tolerance"}
# Per surface (live2 11-17): the general dimension palette's controls, display:none in every hole callout palette
# dump (Tolerance precision, Offset: display:none in the panel and fit dumps too); «Varies» only as the read-only Hole class value with Fit type User defined, which no recorder reads.
# live2 11-17: what full-page dumps carry besides the surface (the page's chrome, the document's and account's names,
# a menu clicked earlier, a transient message); the hole table cell editor's Formulas / Data Types (display:none);
# Edit hatch's Apply to (display:none in every dump; dx 1735: none with a single-part section).
PAGE_CHROME = {"Show measure details ([)", "Show analysis tools", "Display mass and section properties", "Create version…", "Version graph", "Set to last viewed configuration"}
NEVER_SHOWN_IN = {
    "Hole/thread callout": {"Tolerance precision", "Offset", "Precision:", "Tolerance: None", "Shaft class", "Reset text position", "Parentheses", "Toggle radius/diameter dimension", "Varies", "«Varies»"},
    "Custom table": PAGE_CHROME | {"Switch to Custom tables : pvz 2parts", "Switch to fs tables", "pvz 2parts", "onshape cone", "sketch start test"},
    "Insert DXF or DWG": PAGE_CHROME,
    "Move to sheet / Replicate": {"Hide auto centermarks", "Switch to asb drawing src", "Undo (ctrl+z)", "Redo (ctrl+y)", "Update from this workspace (ctrl+q)"},
    # The page's hidden Inspection table, Styles and MBD status panels (their own surfaces count them).
    "Drawing properties": {"Inspection table", "View:", "All sheets", "Current sheet", "All models", "Styles", "Revert to drawing properties", "MBD status", "Model:"},
    "Create Drawing": {"Creating drawing…"},
    "Hole table": {"Formulas", "Data Types"},
    "Hatch region": {"Apply to", "All instances in all views"},
}


def texts_of(path, visible_only=False):
    """Every visible text, option, tooltip and placeholder of a dump (dx's audit reader)."""
    raw = open(path, errors="ignore").read()
    # A recorder that found nothing writes "none" (live drawing-linestyle-colorpicker.txt).
    if raw.strip() == "none":
        return []
    # An HTML comment is no text a user sees (live's reference manager dump: "<!-- Version graph-->").
    raw = re.sub(r"<!--.*?-->", "", raw, flags=re.S)
    if visible_only and path.endswith(".html"):
        raw = strip_hidden(raw)
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
        if USER.search(t) or t in NEVER_SHOWN:
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
    ap.add_argument("--ours-menus", help="our menus per entity (the MENUDUMP json of drawing-ui's drawing_context_menus_are_lives test)")
    args = ap.parse_args()
    src = sources()
    shown = ours_shown(args.run)

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
        # A hovered button (Undo, Redo, Update: never an active tool): its tooltip's title, ours against live's.
        tip, otip = st.get("tooltip"), ost.get("tooltip")
        if isinstance(on, dict) and isinstance(on.get("tool"), str) and (ls.get("do") or {}).get("do") == "hover" and isinstance(tip, str):
            title_of = lambda t: norm(t.split(" / ")[0]) if isinstance(t, str) else None
            tool_seen[key(on["tool"])][0 if title_of(tip) == title_of(otip) else 1] += 1
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
    ours_menus = json.load(open(args.ours_menus)) if args.ours_menus else {}
    for entity, items in menus(args.dom).items():
        # Our menu of that entity (an entity name's words after the first, e.g. "dimension (with an inspection item)").
        mine = ours_menus.get(entity.split(" ")[0])
        mine_keys = {k for r in mine or [] for k in menu_keys(r)}
        for it in items:
            k = menu_key(it)
            if mine is not None:
                st = "=" if k in mine_keys else ("≠" if in_sources(src, it) else "missing")
                if st != "=" and (entity, it) in ACCEPTED_MENU:
                    st = "accepted"
                rows.append((f"Menu: {entity}", it, st, ACCEPTED_MENU.get((entity, it), "our menu")))
            else:
                rows.append((f"Menu: {entity}", it, status(menu_seen.get(k), menu_row_in_sources(src, it)), ""))
        # A row of ours live's menu doesn't have.
        for r in mine or []:
            if r != "——" and not menu_keys(r) & {k for x in items for k in menu_keys(x)}:
                name = f"{r} (ours only)"
                note = ACCEPTED_MENU.get((entity, name))
                rows.append((f"Menu: {entity}", name, "accepted" if note else "≠", note or "our menu"))
    elsewhere = {}
    for sf, it, st, _ in rows:
        if st in ("=", "≠") and (sf.startswith(("Toolbar", "Menu:"))):
            k = key(re.sub(r"\s*\([^)]*\)$", "", it.removesuffix(" (ours only)")))
            if not it.endswith(" (ours only)") and elsewhere.get(k) != "≠":
                elsewhere[k] = st
                elsewhere[key(it)] = st
    for surface, globs_, titles in SURFACES:
        texts = []
        for g in globs_:
            for p in sorted(glob.glob(f"{args.dom}/{g}")):
                hidden = {norm(x) for x in NEVER_SHOWN_IN.get(surface, ())}
                texts += [t for t in texts_of(p, surface in HIDDEN_IS_ABSENT) if t not in texts and t not in hidden]
        recorded = {t for (title, t) in text_seen if title in titles}
        for t in sorted(recorded):
            if NOISE.match(t) or USER.search(t) or len(t) < 2:
                continue
            seen = [sum(text_seen[(ti, t)][i] for ti in titles if (ti, t) in text_seen) for i in (0, 1)]
            rows.append((surface, t, status(seen, in_sources(src, t)), "recorded"))
        mine = set().union(*(v for k, v in shown.items() if any(k == ti or k.startswith(ti) for ti in titles + OURS_TITLES.get(surface, []))))
        # A right-click menu live's panel dump caught open: our menus' rows (the MENUDUMP of our build).
        mine |= {norm(r.removesuffix(" ▸")) for rows_ in ours_menus.values() for r in rows_} | shown.get(MENUS, set())
        # A submenu's rows too ("Tangent edges ▸ Hidden | Solid | Phantom"), its name without the badge.
        for rows_ in ours_menus.values():
            for r in rows_:
                name, arrow, subs = r.replace("\tbeta", "").partition(" ▸ ")
                if arrow:
                    mine |= {norm(name)} | {norm(x) for x in subs.split(" | ") if x.strip()}
        for t in texts:
            if t in recorded:
                continue
            st = status(None, in_sources(src, t))
            # A full-page dump's toolbar button or a right-click row live's menu dumps list: that item's status.
            if st == "present" and key(t) in elsewhere:
                rows.append((surface, t, elsewhere[key(t)], "dump (toolbar / menu item)"))
                continue
            # Live's dump has it and our build showed it in that panel (no take compares the two states).
            # Our brand where live names its own ("Show Onshape drawing templates").
            ours_t = norm(t).replace("Onshape", "Noshape")
            rows.append((surface, t, "shown" if st == "present" and (norm(t) in mine or ours_t in mine) else st, "dump"))

    # Cloud-only rows (out of scope) and dump texts live itself no longer shows (docs/parity/audit-drawing-dataexchange.md).
    rows = [(sf, it, "n/a" if it in CLOUD else ("accepted" if it in ACCEPTED and st != "=" else st), ACCEPTED.get(it, note) if st not in ("=", "accepted") else note)
            for sf, it, st, note in rows]
    kinds = ["=", "shown", "≠", "present", "missing", "accepted", "n/a"]
    counts = defaultdict(lambda: defaultdict(int))
    for sf, _, st, _ in rows:
        group = "Toolbar" if sf.startswith("Toolbar") else "Right-click menus" if sf.startswith("Menu:") else "Panels and dialogs"
        counts[group][st] += 1
        counts["All"][st] += 1
    print("| surface | items | = matching | shown (live's dump, ours' panel) | ≠ differs | present, not compared | missing | accepted (live has none) | n/a (cloud) |")
    print("|---|---|---|---|---|---|---|---|---|")
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
    print("| surface | items | = | shown | ≠ | present, not compared | missing | accepted | ≠, missing and accepted items | not compared (first 10) |")
    print("|---|---|---|---|---|---|---|---|---|---|")
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
        print(f"| {sf} | {len(rs)} | {c['=']} | {c['shown']} | {c['≠']} | {c['present']} | {c['missing']} | {c['accepted']} | {odd} | {'; '.join(todo[:10])}{more} |")
    if args.details:
        print("\n### Every item\n\n| surface | item | status | source |\n|---|---|---|---|")
        for sf, it, st, note in rows:
            print(f"| {sf} | {it} | {st} | {note} |")

if __name__ == "__main__":
    raise SystemExit(main())
