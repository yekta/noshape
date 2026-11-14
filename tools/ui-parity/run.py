#!/usr/bin/env python3
"""Interaction parity suite runner (tests/ui-parity/README.md).

Runs scenarios in noshape (NOSHAPE_UI_SCRIPT), then compares each recording with
live's (tests/ui-parity/live/<name>.json) step by step and names the first step
and key that differ. Prints one line per scenario and the score.

  tools/ui-parity/run.py --app $CARGO_TARGET_DIR/debug/noshape [--display :102] [names...]
  tools/ui-parity/run.py --compare-only            # recordings already in --out

Without names: every scenario that has a live recording (--all: every scenario).
On X11 without a window manager the app's window is focused with xdotool, so
fields get focus events as on a desktop.
"""

import argparse
import json
import re
import os
import subprocess
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CAUSES = {}
KEYS = ["focus", "dialog", "list", "selection", "features", "parts", "message", "tooltip"]
# Added later (README "undo", "sketch", "assembly"): compared once live's recording has them.
LATER_KEYS = ["undo", "sketch", "assembly", "drawing", "document", "documents", "preferences", "variables", "featurestudio", "camera", "measure"]
# Read from the main page, which a drawing's iframe leaves alone (README "### Drawings"): not compared there.
NOT_IN_DRAWINGS = {"selection", "undo", "assembly"}


def remembered_settings(seed):
    """A scenario's "remembered" (what live's account last committed per feature type, behaviours 1987) as the app's
    feature_settings.json: type → group ("", "length:Millimeter", "angle:Degree") → parameter → value. Plain values:
    a list is a Lookup path, a bool a Bool, a string in a length / angle group an expression, {"text": …} a Text (a
    reference, e.g. Form's "library:circle-emboss"), else an Enum."""
    def value(group, v):
        if isinstance(v, dict) and set(v) == {"text"}:
            return {"t": "Text", "v": v["text"]}
        if isinstance(v, list):
            return {"t": "Lookup", "v": v}
        if isinstance(v, bool):
            return {"t": "Bool", "v": v}
        if group or isinstance(v, (int, float)):
            return {"t": "Expr", "v": {"text": str(v)}}
        return {"t": "Enum", "v": v}
    return {ty: {g: {p: value(g, v) for p, v in params.items()} for g, params in groups.items()} for ty, groups in seed.items()}


def run_one(app, scenario, out, display, timeout):
    env = dict(os.environ)
    config = tempfile.mkdtemp(prefix="noshape-uip-config-")
    seed = json.loads(Path(scenario).read_text()).get("remembered")
    if seed:
        Path(config, "feature_settings.json").write_text(json.dumps(remembered_settings(seed)))
        env["NOSHAPE_UI_REMEMBERED"] = "1"
    # Other choices live's account remembers, as the app's own config files (README "config_files").
    for name, content in (json.loads(Path(scenario).read_text()).get("config_files") or {}).items():
        Path(config, name).write_text(json.dumps(content))
    env.update({
        "NOSHAPE_UI_SCRIPT": str(scenario),
        "NOSHAPE_UI_STATE_OUT": str(out),
        "NOSHAPE_CONFIG_DIR": config,
        # The documents page's "Owned by me" (a page start's documents; new documents), never the user's.
        "NOSHAPE_DOCUMENTS_DIR": str(Path(config, "documents")),
        "NOSHAPE_NO_NATIVE_DIALOGS": "1",
        "DISPLAY": display,
    })
    if out.exists():
        out.unlink()
    proc = subprocess.Popen([app], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True)
    t = time.time()
    while proc.poll() is None and time.time() - t < timeout:
        # Focus it until its first frames are up (a window not mapped yet ignores it).
        if display and time.time() - t < 6:
            # The window by its pid, else by its title (one app per display).
            w = subprocess.run(["xdotool", "search", "--pid", str(proc.pid), "--name", "noshape"], env=env, capture_output=True, text=True).stdout.split()
            w = w or subprocess.run(["xdotool", "search", "--onlyvisible", "--name", "noshape"], env=env, capture_output=True, text=True).stdout.split()
            if w:
                subprocess.run(["xdotool", "windowfocus", w[-1]], env=env, capture_output=True)
        time.sleep(0.2)
    if proc.poll() is None:
        proc.kill()
        return "timed out"
    err = proc.stderr.read()
    if not out.exists():
        lines = err.splitlines()
        # A panic first, with its message (the line after "panicked at"), then the steps that ran.
        panic = [" ".join(lines[i:i + 2]) for i, l in enumerate(lines) if "panicked" in l][:1]
        return "no recording: " + " | ".join(panic + [l for l in lines if "UI-SCRIPT" in l])[:600]
    return None


# Live's recorder reads a selection field's invalid state (behaviours "psui (2144)") in its recordings from 2026-11-06
# 10:52 on; before, it always read "ok", so older recordings don't say.
QUERY_STATE_SINCE = "2026-11-06T10:50"


# Live's recorder reads the Section view panel and the appearance editor as dialogs in its recordings from 2026-11-06
# 15:20 on; before, they were no dialog, so ours isn't compared against those.
PANEL_DIALOGS_SINCE = "2026-11-06T15:00"


def is_panel_dialog(d):
    t = (d or {}).get("title") if isinstance(d, dict) else None
    return t in ("Section view", "Appearance") or bool(re.match(r"Face appearance \d+$", str(t or "")))


# Live's recorder reads Select other and the Folder name popover as panels in its recordings from 2026-11-12 on (the
# live2 recorder); before, they were no dialog.
POPOVER_PANELS_SINCE = "2026-11-12T00:00"


def is_popover_panel(d):
    return isinstance(d, dict) and d.get("kind") == "panel" and d.get("title") in ("Select other", "Folder name")


def forget_old_query_states(live):
    """Drop the selection fields' `state` from a recording made before live's recorder could read it."""
    if str(live.get("recorded", "")) >= QUERY_STATE_SINCE:
        return live
    for step in [{"state": live.get("start")}] + live.get("steps", []):
        for d in [(step.get("state") or {}).get("dialog")] + list((step.get("state") or {}).get("dialogs") or []):
            for f in (d or {}).get("fields", []) if isinstance(d, dict) else []:
                # (An array's box too: live ui-parity loft-dialog's Profiles read "ok" then.)
                if isinstance(f, dict) and f.get("kind") in ("query", "other"):
                    f.pop("state", None)
    return live


def same_position(path, k, a, b):
    """A drawing view's or annotation's sheet position (live2 11-12, sheet mm): equal within 2 mm, compared where both
    sides have one."""
    if k != "position" or not path.startswith(".drawing"):
        return False
    pa, pb = a[k], b[k]
    return pa is None or pb is None or (len(pa) == len(pb) and all(abs(x - y) <= 2.0 for x, y in zip(pa, pb)))


def first_difference(a, b, path=""):
    """The first key path where `a` (live) and `b` (ours) differ, with both values. Keys only one side records
    (a side's debugging aids) are not compared; the selection compares its count only until both record kind and owner."""
    if isinstance(a, dict) and isinstance(b, dict):
        for k in a:
            if k not in b:
                continue
            # Create Drawing's name field: live's recorder reads it from 11-08 on (older recordings list no fields).
            if path == ".dialog" and k == "fields" and a.get("title") == "Create Drawing: New drawing name" and a[k] == []:
                continue
            # Live's recorder couldn't read the sketch (`"error": "sketch model not reached"`, before a plane is picked).
            if path == ".sketch" and a.get("error") and k in ("entities", "constraints", "dimensions"):
                continue
            if path == ".selection" and k == "items" and not all("owner" in i and i.get("kind") for i in (a[k] or [])):
                continue
            # A flip button has no label of its own (live records its icon's name).
            if k == "label" and a.get("kind") == "toggle":
                continue
            if same_position(path, k, a, b):
                continue
            d = first_difference(a[k], b[k], f"{path}.{k}")
            if d:
                return d
        return None
    if isinstance(a, list) and isinstance(b, list):
        for i, (x, y) in enumerate(zip(a, b)):
            d = first_difference(x, y, f"{path}[{i}]")
            if d:
                return d
        if len(a) != len(b):
            return (f"{path}.length", len(a), len(b))
        return None
    return None if a == b else (path, a, b)


def all_differences(a, b, path=""):
    """Every key path where `a` (live) and `b` (ours) differ (the rules of first_difference)."""
    if isinstance(a, dict) and isinstance(b, dict):
        out = []
        for k in a:
            if k not in b or (path == ".selection" and k == "items" and not all("owner" in i and i.get("kind") for i in (a[k] or []))):
                continue
            if k == "label" and a.get("kind") == "toggle":
                continue
            if same_position(path, k, a, b):
                continue
            out += all_differences(a[k], b[k], f"{path}.{k}")
        return out
    if isinstance(a, list) and isinstance(b, list):
        out = [d for i, (x, y) in enumerate(zip(a, b)) for d in all_differences(x, y, f"{path}[{i}]")]
        return out + ([(f"{path}.length", len(a), len(b))] if len(a) != len(b) else [])
    return [] if a == b else [(path, a, b)]


# Cloud features noshape leaves out (AGENTS.md: comments, sharing…): not compared in live's menus.
# Live's info bubbles hide by themselves after ≈4.4 s (live TOAST-AUTOHIDE; app workspace.rs).
TOAST_MS = 4400
INPUT_BUTTONS = {"Apply", "Cancel", "Save", "OK"}

# Tooltips live draws as an element's native `title` (the OS's tooltip, which its recorder can't read): ours' is read as
# none while live's is (dom/panel-versions-history.txt: a history row's tip="Open version V1").
# The merge dialog's ⓘ too (live2 ver-merge-drawing-info: hovered, read as none).
NATIVE_TITLE_TOOLTIPS = re.compile(r"^(Open (version|workspace) |For this tab type, edits from separate branches cannot be merged\.$)")
CLOUD_ONLY = {"View support tickets", "App Store dev portal", "Sign out", "Add comment", "Copy link", "Share…", "Transfer ownership…", "Applications", "Create CAM Studio", "Create PCB Studio",
              "Create Render Studio", "Create tutorial…", "Explore Onshape", "What's new", "Forums", "Report a bug",
              "Import from", "Create task…"}
# Rows live showed for a while and no longer has (live 2336: the + menu's "Create table" only in its 05:00–06:20
# recordings, in no document since).
# After live's last Free-plan take (2026-11-13 19:30; its first Professional reading 2026-11-14 02:13, live2). Free
# takes list Release management's rows with an Upgrade badge in every menu; a Professional company shows them only
# with managed workflows on (ours by default off): those rows aren't compared in Free takes.
LIVE_PROFESSIONAL_FROM = "2026-11-13T19:31:00Z"
FREE_PLAN_ROWS = {"Release…", "Releases", "Revision history", "Revision history…"}


# Live's recorder reads a drawing's open part picker (Select a part or assembly) as a feature dialog since 11-14;
# before, the panel beside it (ours keeps that reading in the picker's `panel`).
PICKER_READ_SINCE = "2026-11-14T00:00:00Z"


def picker_as_read(state, since):
    """Ours with the drawing picker's dialog as the take's recorder read it."""
    dlg = (state or {}).get("dialog")
    if isinstance(dlg, dict) and "panel" in dlg:
        state["dialog"] = {k: v for k, v in dlg.items() if k != "panel"} if since else dlg["panel"]
    return state


def without_free_plan_rows(state):
    """`state` with Release management's menu rows dropped (a Free-plan take's and ours)."""
    lst = (state or {}).get("list")
    if isinstance(lst, dict) and any(r in FREE_PLAN_ROWS for r in lst.get("rows") or []):
        rows = lst["rows"]
        hl = lst.get("highlighted")
        hl_row = rows[hl] if isinstance(hl, int) and 0 <= hl < len(rows) else None
        lst["rows"] = [r for r in rows if r not in FREE_PLAN_ROWS]
        lst["highlighted"] = kept_index(rows, hl, FREE_PLAN_ROWS, lst["rows"], hl_row)
        if isinstance(lst.get("disabled"), list):
            kept = [i for i, r in enumerate(rows) if r not in FREE_PLAN_ROWS]
            lst["disabled"] = [kept.index(i) for i in lst["disabled"] if i in kept]
    return state

LIVE_TRANSIENT_ROWS = {"Create table"}
# Rows of live's Professional plan (the research account since 2026-11-14): ours always has them (offline, no plan);
# older takes, recorded on the free plan, don't.
PRO_SINCE = "2026-11-14T00:00"
PRO_ROWS = {"Where used…"}


def normalize_live(state):
    """Live's state in the agreed form: cloud-only menu rows dropped, no highlighted row as null (live: -1)."""
    # A focused checkbox: the DOM gives its value "on" and no selection; it has no text.
    focus = (state or {}).get("focus")
    if isinstance(focus, dict) and focus.get("text") == "on" and focus.get("selection") == [None, None]:
        focus["text"], focus["selection"] = None, None
        # A clicked checkbox (an <input> with no selection): no text field (README "Focus").
        if focus.get("tag") == "input":
            focus["tag"] = "checkbox"
    # The Tabs panel's Sort menu hangs inside the panel's header: live's recorder reads its rows into the panel's name.
    doc, lst = (state or {}).get("document"), (state or {}).get("list")
    if isinstance(doc, dict) and isinstance(lst, dict) and isinstance(doc.get("panel"), str) and lst.get("rows"):
        if doc["panel"] == "Tabs " + " ".join(lst["rows"]):
            doc["panel"] = "Tabs"
    # The Feature Studio's code editor: Ace's hidden textarea holds a piece of the text around the caret, not a field's
    # text; the editor's own `featurestudio.caret` is compared instead.
    if isinstance(focus, dict) and focus.get("tag") == "textarea" and (state or {}).get("featurestudio"):
        state["focus"] = {"kind": "other"}
    # Live's recorder leaves an instance's owner empty when a command selected it (Select instances with same
    # configuration, Dynamic suppression's panel), not a click: compared by kind.
    sel = (state or {}).get("selection")
    if isinstance(sel, dict):
        for it in sel.get("items") or []:
            if isinstance(it, dict) and it.get("kind") == "INSTANCE" and it.get("owner") is None and it.get("id") != "assemblyRoot":
                it.pop("owner", None)
            # Nor a sketch's entities the step itself made (Paste sketch: the new sketch isn't in its list yet when it
            # reads, sketch-row-copy-paste-onto-front).
            if isinstance(it, dict) and it.get("kind") == "SKETCH_ENTITY" and it.get("owner") is None:
                it.pop("owner", None)
    # Ctrl+Space's card: an empty one (its div stays in the page) is none (live 2414).
    doc = (state or {}).get("document")
    if isinstance(doc, dict) and isinstance(doc.get("switcher"), dict) and not doc["switcher"].get("items"):
        doc["switcher"] = None
    # The bubble's close button is part of live's text ("Select a sketch plane × Close").
    if isinstance((state or {}).get("message"), str):
        state["message"] = state["message"].removesuffix("× Close").strip()
    # A flip button's state is its icon in live's DOM ("…-opposite": not flipped); its "checked" isn't kept.
    for f in ((state or {}).get("dialog") or {}).get("fields") or []:
        if f.get("kind") == "toggle" and str(f.get("label", "")).startswith("icon:"):
            f["checked"] = not str(f["label"]).endswith("-opposite")
            # (Its recorders before 2026-11-07 named it "toggle", as ours does.)
            f["label"] = "toggle"
    # Live's runner opens a copy of the start's Assembly, whose root row can read "<name> Copy 1" while it opens.
    for r in (((state or {}).get("features") or {}).get("rows") or []):
        if "is-instance" in str(r.get("cls", "")) and isinstance(r.get("name"), str):
            r["name"] = re.sub(r" Copy \d+$", "", r["name"])
    # The sketch model is read from live's page once a canvas event touched it: before, its lists are null
    # ("not recorded", not empty) and aren't compared.
    sk = (state or {}).get("sketch")
    if isinstance(sk, dict):
        for k in ("entities", "constraints", "dimensions"):
            if sk.get(k) is None:
                sk.pop(k, None)
        sk.pop("error", None)
    # A drawing live's recorder couldn't read (only its `error`, e.g. after Switch to reference left the drawing tab:
    # live2 drw-sheet-ref-switch) isn't compared.
    dr = (state or {}).get("drawing")
    if isinstance(dr, dict) and set(dr) == {"error"}:
        state.pop("drawing")
    # Live's runner works in a copy of the start, its Part Studio "start <name> Copy 1" in a drawing panel.
    d = (state or {}).get("dialog")
    if isinstance(d, dict) and d.get("kind") == "drawing-panel":
        d["text"] = [re.sub(r"^start \S+ Copy \d+( |$)", r"Part Studio 1\1", t) if isinstance(t, str) else t for t in d.get("text") or []]
    # The same name in a modal's File name ("start block Copy 1 - Sketch 1": Export as DXF/DWG; an assembly's instance
    # Export…: "start two-blocks - Part 2"), and as the focus' text.
    focus = (state or {}).get("focus")
    if isinstance(focus, dict) and isinstance(focus.get("text"), str) and re.match(r"^start \S+ Copy \d+ - ", focus["text"]):
        n = len(focus["text"])
        focus["text"] = re.sub(r"^start \S+ Copy \d+ - ", "Part Studio 1 - ", focus["text"])
        if focus.get("selection") == [0, n]:
            focus["selection"] = [0, len(focus["text"])]
    if isinstance(d, dict) and d.get("kind") == "modal" and isinstance(d.get("fields"), list):
        # Live's recorder reads an <input type="button"> (Workspace protection's Apply / Cancel) as a field: a button.
        is_button = lambda f: isinstance(f, dict) and f.get("label") is None and f.get("kind") == "input" and f.get("value") in INPUT_BUTTONS
        buttons = [f["value"] for f in d["fields"] if is_button(f)]
        if buttons:
            d["fields"] = [f for f in d["fields"] if not is_button(f)]
            d["buttons"] = (d.get("buttons") or []) + [b for b in buttons if b not in (d.get("buttons") or [])]
    if isinstance(d, dict) and d.get("kind") == "modal":
        # A modal titled after the copy ("Input propagation for start two-blocks asm Copy 2").
        if isinstance(d.get("title"), str):
            d["title"] = re.sub(r" Copy \d+$", "", d["title"])
        for f in d.get("fields") or []:
            if isinstance(f, dict) and str(f.get("label", "")).startswith("File name") and isinstance(f.get("value"), str):
                f["value"] = re.sub(r"^start \S+( Copy \d+)? - ", "Part Studio 1 - ", f["value"])
    # The Properties modal as live's recorder reads it since 2026-11-08 (Description, the category row labelled by its
    # value, Undo / Redo named), in its earlier form, ours' (part-menu-properties): no Description, "Category", the
    # icon buttons unnamed.
    d = (state or {}).get("dialog")
    if isinstance(d, dict) and d.get("title") == "Properties" and isinstance(d.get("fields"), list):
        f = [x for x in d["fields"] if not (isinstance(x, dict) and x.get("label") == "Description")]
        for x in f:
            if isinstance(x, dict) and str(x.get("label", "")).startswith("Onshape "):
                x["label"], x["value"] = "Category", ""
        # Live's recorder from 2026-11-13.1 reads a part's Mass group first (its Override checkbox and the greyed
        # computed value, parts-multi-properties); ours' "Mass Override" checkbox comes last.
        o = [x for x in f[:2] if isinstance(x, dict) and x.get("label") == "Override"]
        if len(o) == 2 and o[0].get("kind") == "checkbox" and o[1].get("kind") == "input":
            f = f[2:] + [dict(o[0], label="Mass Override")]
        d["fields"] = f
        for k in ("buttons", "disabledButtons"):
            if isinstance(d.get(k), list):
                d[k] = ["" if b in ("Undo", "Redo") else b for b in d[k]]
    # Live's Free plan makes public documents: "Create public document" (New document, Copy workspace / version) is a
    # paid plan's and ours' "Create document".
    if isinstance(d, dict) and isinstance(d.get("buttons"), list):
        d["buttons"] = ["Create document" if b == "Create public document" else b for b in d["buttons"]]
    # A REST-built start's history reads "Add feature" where the UI's (ours) reads "Insert feature" (live2 2258).
    doc = (state or {}).get("document")
    if isinstance(doc, dict):
        for h in doc.get("history") or []:
            if isinstance(h, dict) and isinstance(h.get("name"), str):
                h["name"] = h["name"].replace(" :: Add feature : ", " :: Insert feature : ")
        cmp = doc.get("compare")
        if isinstance(cmp, dict):
            for k in ("target", "source"):
                if isinstance(cmp.get(k), str):
                    cmp[k] = cmp[k].replace(" :: Add feature : ", " :: Insert feature : ")
    # A studios run's document is "live2 uip <start> <time>" in live, "<start>" in ours (the navbar's rename field).
    focus = (state or {}).get("focus")
    if isinstance(focus, dict) and isinstance(focus.get("text"), str) and re.match(r"^live2 uip \S+ \d{4}-\d\d-\d\dT[\d:]+", focus["text"]):
        n = len(focus["text"])
        focus["text"] = re.sub(r"^live2 uip (\S+) \d{4}-\d\d-\d\dT[\d:]+", r"\1", focus["text"])
        if focus.get("selection") == [0, n]:
            focus["selection"] = [0, len(focus["text"])]
    lst = (state or {}).get("list")
    if isinstance(lst, dict):
        rows = lst.get("rows") or []
        hl = lst.get("highlighted")
        hl_row = rows[hl] if isinstance(hl, int) and 0 <= hl < len(rows) else None
        lst["rows"] = [r for r in rows if r not in CLOUD_ONLY | LIVE_TRANSIENT_ROWS]
        # A panel's ⋯ menu: live's recorder names it "menu" (ours "context-menu": the same rows to a user), and in the
        # assembly's panels reads each row twice (asm-bom-copy-table: "Copy table", "Copy table", …).
        if lst.get("kind") == "menu":
            lst["kind"] = "context-menu"
            r = lst["rows"]
            if r and len(r) % 2 == 0 and all(r[i] == r[i + 1] for i in range(0, len(r), 2)):
                lst["rows"] = r[::2]
        # An assembly start's names in menu rows: live's Part Studio is "start <name>" (ours "Part Studio 1") and its
        # runner works in a copy of the assembly ("start <name> asm Copy 2").
        lst["rows"] = [re.sub(r" asm Copy \d+…$", " asm…", re.sub(r"^Switch to start \S+$", "Switch to Part Studio 1", r)) if isinstance(r, str) else r for r in lst["rows"]]
        lst["highlighted"] = kept_index(rows, hl, CLOUD_ONLY | LIVE_TRANSIENT_ROWS, lst["rows"], hl_row)
        # A row fewer, the menu sits elsewhere when it has to move to fit (live's flipped up under the still pointer,
        # ours fitted below it: face-rclick-menu): which row the pointer is over isn't compared then.
        if len(lst["rows"]) != len(rows):
            lst["moved"] = True
    return state


def quantity(text):
    """(number, unit) of a value box's text ("29.73371 mm"), else None."""
    m = re.fullmatch(r"\s*(-?[0-9.]+)\s*([a-z°]*)\s*", str(text or ""))
    return (float(m.group(1)), m.group(2)) if m else None


def close(x, y, rel=0.1):
    return abs(x - y) <= rel * max(abs(x), abs(y), 1e-9)


def drawn_sizes(a, b):
    """Sizes that come from where a click landed (a drawn line's length in its value box or dimension, before a value is
    typed) differ by a pixel or two between the two sides: within 10 % ours reads as live's (README "Comparison")."""
    fa, fb = (a or {}).get("focus"), (b or {}).get("focus")
    if isinstance(fa, dict) and isinstance(fb, dict) and fa.get("kind") == "other" == fb.get("kind"):
        qa, qb = quantity(fa.get("text")), quantity(fb.get("text"))
        if qa and qb and qa[1] == qb[1] and close(qa[0], qb[0]):
            fb["text"], fb["selection"] = fa.get("text"), fa.get("selection")
    da = ((a or {}).get("sketch") or {}).get("dimensions")
    db = ((b or {}).get("sketch") or {}).get("dimensions")
    if isinstance(da, list) and isinstance(db, list):
        for x, y in zip(da, db):
            if x.get("kind") == y.get("kind") and isinstance(x.get("value"), (int, float)) and isinstance(y.get("value"), (int, float)) and close(x["value"], y["value"]):
                y["value"] = x["value"]


def placed_alike(a, b):
    """An instance's transform after a drag comes from where the pointer landed: within 0.5 mm and 0.002 (rotation
    entries) ours reads as live's (README "Comparison")."""
    ia = ((a or {}).get("assembly") or {}).get("instances")
    ib = ((b or {}).get("assembly") or {}).get("instances")
    if not (isinstance(ia, list) and isinstance(ib, list)):
        return
    # Live reads them over REST, in no order the list shows: compared by name.
    ia.sort(key=lambda i: str(i.get("name")))
    ib.sort(key=lambda i: str(i.get("name")))
    for x, y in zip(ia, ib):
        ta, tb = x.get("transform"), y.get("transform")
        if isinstance(ta, list) and isinstance(tb, list) and len(ta) == len(tb) == 16:
            tol = [0.5 if k in (3, 7, 11) else 0.002 for k in range(16)]
            if all(isinstance(u, (int, float)) and isinstance(w, (int, float)) and abs(u - w) <= t for u, w, t in zip(ta, tb, tol)):
                y["transform"] = ta


def cloud_menu(a, b):
    """A menu that lost live's cloud-only rows ([`normalize_live`]): the row under a pointer that didn't move isn't
    compared (the shorter menu may fit where live's had to move under the pointer)."""
    la, lb = (a or {}).get("list"), (b or {}).get("list")
    if isinstance(la, dict) and la.pop("moved", False) and isinstance(lb, dict):
        lb["highlighted"] = la.get("highlighted")


# The scenarios' start documents (starts.json), read in main.
STARTS = {}


# Rows only a local app has (a document's file in the OS's file manager, the command line and coding agents; user 11-07):
# not compared.
LOCAL_ROWS = {"Show in folder", "Show in Explorer", "Show in Finder", "Install command line tool…", "Connect to coding agents…"}


# Rows that depend on what the clipboard held before the scenario (live's runner keeps one browser session, so a copy
# from an earlier scenario shows Paste into sketch in a later one): compared only in scenarios that copy something.
CLIPBOARD_ROWS = {"Paste into sketch", "Paste sketch", "Paste sketch entities", "Paste tab", "Paste inputs"}


def copies(live):
    """Whether a scenario copies something itself (a Copy… menu row or Ctrl/Cmd+C)."""
    for s in live.get("steps", []):
        do = s.get("do") or {}
        menu = str((do.get("on") or {}).get("menu") or "")
        if menu.startswith("Copy") or str(do.get("key", "")).lower() in ("mod+c", "ctrl+c", "cmd+c", "secondary-c"):
            return True
    return False


# Rows only Onshape's paid plans show (live's account is free: its Show analysis tools menu has no Thickness
# analysis); ours has them, as parity is behaviour, not a plan's limits. Not compared.
# The account menu's Company settings: a company plan's (live's account is free; ours keeps the company's settings).
PLAN_ROWS = {"Thickness analysis…", "Edit thickness analysis", "Turn thickness analysis off", "Company settings"}


def fold_start_edits(state):
    """A start's sketch is one history row in live's REST-built starts (and after a sketch's ✓ in the UI); ours' start
    builder adds its shapes in a second edit: its "<tab> :: Edit : <sketch>" right after "<tab> :: Insert feature :
    <sketch>" is folded into that row (on both sides)."""
    doc = (state or {}).get("document")
    rows = doc.get("history") if isinstance(doc, dict) else None
    if not isinstance(rows, list):
        return state
    # (Newest first: the edit is the row before its insert.)
    names = [r.get("name") if isinstance(r, dict) else None for r in rows]
    doc["history"] = [r for i, r in enumerate(rows) if not (isinstance(names[i], str) and " :: Edit : " in names[i] and i + 1 < len(rows) and names[i + 1] == names[i].replace(" :: Edit : ", " :: Insert feature : "))]
    return state


TEXT_TAGS = {"input", "textarea", "select"}


def no_field_focus(focus):
    """Whether no text field has the keyboard: "none", or "other" on a button, a row, a menu (live: Chrome focuses a
    clicked button; ours' buttons take no focus), or a feature dialog's button or link ("field" with tag button / a:
    vs-insert-help, vs-insert-version-graph), or a checkbox. Two such are compared as equal (README "Focus")."""
    if not isinstance(focus, dict):
        return focus is None
    if focus.get("kind") == "none":
        return True
    if focus.get("kind") == "field" and focus.get("tag") in ("button", "a") and not focus.get("text"):
        return True
    # A native <select> holding the keyboard (its value is the field's state, compared there; live reads Angular's
    # "0: hertz" as its text: doc-units-workspace-units-frequency-unit, prefs-units-frequency).
    if focus.get("tag") == "select":
        return True
    # A checkbox (an <input> whose value reads "on", no text selection: live ver-branch-no-open).
    if focus.get("tag") == "input" and focus.get("text") in ("on", "off") and focus.get("selection") in (None, [None, None]):
        return True
    return focus.get("kind") == "other" and focus.get("tag") not in TEXT_TAGS and not focus.get("text") and focus.get("selection") in (None, [None, None])


def same_fieldless_focus(a, b):
    """Live's and ours' focus when neither has a text field: ours takes live's (the rule above)."""
    if isinstance(a, dict) and isinstance(b, dict) and no_field_focus(a.get("focus")) and no_field_focus(b.get("focus")):
        b["focus"] = a.get("focus")
    return b


def sorted_child_rows(fields):
    out, run = [], []
    for f in fields + [None]:
        if isinstance(f, dict) and f.get("child") is True:
            run.append(f)
            continue
        out += sorted(run, key=lambda x: str(x.get("label")))
        run = []
        if f is not None:
            out.append(f)
    return out


def drop_rows(state, drop):
    lst = (state or {}).get("list")
    if not isinstance(lst, dict) or lst.get("kind") != "context-menu":
        return state
    rows = lst.get("rows") or []
    hl = lst.get("highlighted")
    hl_row = rows[hl] if isinstance(hl, int) and 0 <= hl < len(rows) else None
    lst["rows"] = [r for r in rows if r not in drop]
    lst["highlighted"] = kept_index(rows, hl, drop, lst["rows"], hl_row)
    return state


def kept_index(rows, hl, dropped, kept, hl_row):
    """The highlighted row's index once `dropped` rows are gone: by position (rows can repeat: a function's overloads,
    fs-ac-fn-overloads), else by its text after a renaming; None when it was dropped."""
    if isinstance(hl, int) and 0 <= hl < len(rows) and rows[hl] not in dropped and len(kept) == sum(1 for r in rows if r not in dropped):
        return sum(1 for r in rows[:hl] if r not in dropped)
    return kept.index(hl_row) if hl_row in kept else None


# The Insert dialog's fixed text (live reads its rows too, but its lane document holds other runs' studios).
INSERT_FIXED = {"Current document", "Other documents", "Standard content", "Part Studios", "Assemblies", "(Search Part Studios)", "(Search assemblies)"}
# Panels live's recorder read only from these times on: ours isn't compared against older takes there.
PANELS_READ_SINCE = {"Insert parts and assemblies": "2026-11-09T00:00", "Named views": "2026-11-12T00:00", "Material": "2026-11-12T00:00", "Dependencies of": "2026-11-12T00:00"}


def panel_read_since(title):
    """When live's recorder started reading a panel (a "Dependencies of …" panel by its prefix)."""
    title = str(title or "")
    return PANELS_READ_SINCE.get(title) or next((v for k, v in PANELS_READ_SINCE.items() if k.endswith(" of") and title.startswith(k + " ")), "")


def reference_manager_as_panel(a, b):
    """Live's recorder reads the Reference manager as a panel of texts in its recordings from 2026-11-10 on (before, as a
    feature dialog with tabs and fields): ours' dialog put in that form when live's is."""
    da, db = (a or {}).get("dialog"), (b or {}).get("dialog")
    if not (isinstance(da, dict) and isinstance(db, dict) and da.get("title") == db.get("title") == "Reference manager"
            and da.get("kind") == "panel" and db.get("kind") == "feature"):
        return b
    text = [t.get("label") for t in db.get("tabs") or []]
    for f in db.get("fields") or []:
        if f.get("kind") == "text":
            text.append(f.get("label"))
            continue
        if f.get("kind") == "boolean":
            text.append("[x] " if f.get("checked") else "[ ] ")
        text.append(f.get("label"))
        text += str(f.get("text") or "").split()
    text.append(db.get("okLabel"))
    return dict(b, dialog={"kind": "panel", "title": "Reference manager", "text": [t for t in text if t is not None]})


def material_as_panel(a, b):
    """Live's recorder reads the Material panel in full (kind "material": tabs, fields, "assigned", the list's choices) in
    its recordings from runner 2026-11-14.11 on; before, as a panel of texts (its tabs, the picker's text, "No material
    assigned"): ours' put in that form when live's is."""
    da, db = (a or {}).get("dialog"), (b or {}).get("dialog")
    if not (isinstance(da, dict) and isinstance(db, dict) and da.get("kind") == "panel" and da.get("title") == "Material"
            and db.get("kind") == "material"):
        return b
    text = ["Library", "Custom"]
    custom = any(t.get("selected") for t in db.get("tabs") or [] if t.get("label") == "Custom")
    if not custom:
        text += [f.get("value") for f in db.get("fields") or [] if f.get("label") == "Material"]
        if db.get("assigned"):
            text.append(db["assigned"])
    return dict(b, dialog={"kind": "panel", "title": "Material", "text": text})


def camera_compared(state):
    """`camera` (live's recorder since 2026-11-12): compared by where it looks from, its view direction and up to 3
    decimals, and its projection; zoom and the window's size are each side's own."""
    c = (state or {}).get("camera")
    if not isinstance(c, dict):
        return state
    r = lambda v: [round(x, 3) + 0.0 for x in v] if isinstance(v, list) else v
    return dict(state, camera={"direction": r(c.get("direction")), "up": r(c.get("up")), "orthographic": c.get("orthographic")})


def unknown_picks(a, b):
    """A start feature's pick built over REST by a point ("edge through [...]") reads "Unknown of Extrude 1" in live's
    chips (fillet-edit-cancel-keeps); a user's pick never does: ours' "Edge of Extrude 1" is the same pick."""
    da, db = (a or {}).get("dialog"), (b or {}).get("dialog")
    if not (isinstance(da, dict) and isinstance(db, dict)):
        return a
    fields = []
    for x, y in zip(da.get("fields") or [], db.get("fields") or []):
        if isinstance(x, dict) and isinstance(y, dict) and isinstance(x.get("items"), list) and isinstance(y.get("items"), list) and len(x["items"]) == len(y["items"]):
            same = lambda u, v: u == v or (isinstance(u, str) and isinstance(v, str) and u.startswith("Unknown of ") and re.fullmatch(r"(Edge|Face|Vertex) of " + re.escape(u[len("Unknown of "):]), v))
            x = dict(x, items=[v if same(u, v) else u for u, v in zip(x["items"], y["items"])])
        fields.append(x)
    return dict(a, dialog=dict(da, fields=fields + (da.get("fields") or [])[len(fields):]))


def old_workspace_labels(a, b):
    """The Create workspace modal's fields read before live's 2026-11-10 recorder (live 2418): the name unlabelled and
    the description by its hint; ours names them as the new one does."""
    da, db = (a or {}).get("dialog"), (b or {}).get("dialog")
    if not (isinstance(da, dict) and isinstance(db, dict) and str(da.get("title", "")).startswith("Create workspace from")):
        return b
    fa, fb = da.get("fields") or [], db.get("fields") or []
    old = {None: "Name", "Description cannot contain HTML.": "Description"}
    fields = [dict(y, label=x.get("label")) if isinstance(x, dict) and isinstance(y, dict) and old.get(x.get("label"), object()) == y.get("label") else y for x, y in zip(fa, fb)] + fb[len(fa):]
    return dict(b, dialog=dict(db, fields=fields))


def insert_panel(state):
    d = (state or {}).get("dialog")
    if isinstance(d, dict) and d.get("kind") == "panel" and d.get("title") == "Insert parts and assemblies":
        d["text"] = [t for t in d.get("text") or [] if t in INSERT_FIXED]
    return state


def runner_names(state, start, live_side):
    """A panel's text naming the runners' own documents (asm-menu-change-configuration, -change-to-version): live2's
    document ("noshape ui parity live2-1") and ours (the start's name) read alike, and live2's Part Studio, which its
    runner names after the start, as ours' "Part Studio 1"."""
    d = (state or {}).get("dialog")
    if not (isinstance(d, dict) and start):
        return state
    def name(t):
        if live_side and isinstance(t, str) and re.fullmatch(r"noshape ui parity( live2-\d+[a-z]?)?", t):
            return "<document>"
        # live2's per-start documents since 11-13 ("live2 uip block-drawing-v1 2026-11-13T19:06:18").
        if live_side and isinstance(t, str) and re.fullmatch(rf"live2 uip {re.escape(start)}( \S+)?", t):
            return "<document>"
        if live_side and t == f"start {start}":
            return "Part Studio 1"
        if not live_side and t == start:
            return "<document>"
        return t
    if d.get("kind") == "panel" and isinstance(d.get("text"), list):
        d["text"] = [name(t) for t in d["text"]]
    # A feature dialog's rows by their labels (the Reference manager's documents, drw-ref-selective-update-pin).
    if d.get("kind") == "feature" and isinstance(d.get("fields"), list):
        for f in d["fields"]:
            if isinstance(f, dict) and "label" in f:
                f["label"] = name(f["label"])
    return state


def drawn_rows_only(a, b):
    """Live's page draws a long feature list's rows on screen only (ver-show-more-changes: 25 of 62): ours is compared
    on those rows, and a rollback bar past them (which live's reader can't reach) isn't compared."""
    fa, fb = (a or {}).get("features"), (b or {}).get("features")
    if not (isinstance(fa, dict) and isinstance(fb, dict)):
        return b
    ra, rb = fa.get("rows") or [], fb.get("rows") or []
    if len(ra) < 20 or len(ra) >= len(rb) or [r.get("name") for r in ra] != [r.get("name") for r in rb[:len(ra)]]:
        return b
    fb = dict(fb, rows=rb[:len(ra)])
    shown = {r.get("name") for r in ra}
    if fa.get("rollbackAfter") is None and fb.get("rollbackAfter") not in shown:
        fb["rollbackAfter"] = None
    return dict(b, features=fb)


def scenario_start(name):
    try:
        return json.loads((ROOT / "tests" / "ui-parity" / f"{name}.json").read_text()).get("start")
    except (OSError, ValueError, TypeError):
        return None


def compare(live, ours, whole_document=True):
    """(matched steps, total steps, first difference or None). `whole_document`: the scenario starts on a whole
    document or a page (README "Documents, tabs and preferences"); otherwise live's runner works in a lane copy whose
    tab bar isn't the start's, so `document` isn't compared."""
    pairs = [("start", live.get("start"), ours.get("start"), None, None, None)]
    # A scenario whose first step clicks a tab: which tab the start shows is the runners' convention (live2's opened
    # on that tab until 11-08, on the start's own since: ver-create-version-drawings-warning), not compared.
    first = (live.get("steps") or [{}])[0].get("do") or {}
    if isinstance(first, dict) and isinstance(first.get("on"), dict) and "doc_tab" in first["on"]:
        pairs = []
    clipboard_free = not copies(live)
    ours_steps = {s.get("step"): s for s in ours.get("steps", [])}
    # Live still loading: a menu click that changed nothing in live but close the menu, followed by a wait for its
    # result (the pasted tab of doc-tab-menu-copy-paste shows ~1 s later); ours, done at once, is compared after the
    # wait.
    steps = live.get("steps", [])
    loading = set()
    settled = lambda st: {k: v for k, v in (st or {}).items() if k not in ("list", "focus", "tooltip")}
    for i, s in enumerate(steps[:-1]):
        prev = steps[i - 1].get("state") if i > 0 else live.get("start")
        nxt, do = steps[i + 1].get("do") or {}, s.get("do") or {}
        if isinstance(nxt, dict) and nxt.get("do") == "wait" and isinstance(do.get("on"), dict) and "menu" in do["on"] and settled(s.get("state")) == settled(prev):
            loading.add(s.get("step"))
    for s in steps:
        if s.get("step") in loading:
            continue
        o = ours_steps.get(s.get("step"), {})
        # A step ours couldn't do (its target not found) while live did: a difference, whatever the state after.
        failed = o.get("error") if o.get("ok") is False and s.get("ok") is not False else None
        pairs.append((f"step {s.get('step')}", s.get("state"), o.get("state"), failed, s.get("t"), s.get("do")))
    matched, first = 0, None
    # Live's toasts by when they first showed (live's own clock, `t` ms): one live had dropped by its timeout (an
    # info bubble ≈4.4 s, TOAST-AUTOHIDE) while ours still shows it isn't a difference: the runner doesn't wait live's
    # seconds between steps (sketch-dim-menu-unconfigure, feature-menu-edit-expression, vs-cfg-value-unconfigure).
    toast_shown = {}
    # Ours' alerts by whether they first showed at a step live took a toast's lifetime or longer over.
    ours_slow, prev_t = {}, None
    start_name = scenario_start(live.get("scenario")) if isinstance(live.get("scenario"), str) else None
    free_plan = str(live.get("recorded") or "9") < LIVE_PROFESSIONAL_FROM
    picker_read = str(live.get("recorded") or "9") >= PICKER_READ_SINCE
    for name, a, b, failed, t, step in pairs:
        if failed:
            if first is None:
                first = (name, ".step", "ok", f"failed: {failed}")
            CAUSES.setdefault(".step", [0, f"ok / failed: {failed}"])[0] += 1
            continue
        # Live's document runner (run.mjs) blanks the tooltip after every step but a hover; its page runner (a state
        # with `url`: docs-*, prefs-*) reads it.
        tip_blanked = isinstance(a, dict) and "url" not in a and (step or {}).get("do") != "hover"
        a = insert_panel(runner_names(normalize_live(json.loads(json.dumps(a))), start_name, True))
        b = insert_panel(runner_names(old_workspace_labels(a, reference_manager_as_panel(a, material_as_panel(a, json.loads(json.dumps(b))))), start_name, False))
        if free_plan:
            a, b = without_free_plan_rows(a), without_free_plan_rows(b)
        b = picker_as_read(b, picker_read)
        a = unknown_picks(a, b)
        a, b = camera_compared(a), camera_compared(b)
        bl = (b or {}).get("list")
        if isinstance(bl, dict) and isinstance(bl.get("rows"), list) and str(live.get("recorded", "")) < PRO_SINCE:
            b = dict(b, list=dict(bl, rows=[r for r in bl["rows"] if r not in PRO_ROWS]))
        bd = (b or {}).get("dialog")
        if isinstance(bd, dict) and str(live.get("recorded", "")) < panel_read_since(bd.get("title")) and (a or {}).get("dialog") is None:
            b = dict(b, dialog=None)
        # A file picked for an import: ours imports at once, live's upload takes seconds and its "… has been imported"
        # toast shows only in the next wait (doc-file-tab-update, doc-plus-import-png; psui 11-12): not compared at the
        # pick, live's slowness isn't copied.
        if (step or {}).get("do") == "pick_files" and (a or {}).get("message") is None and "has been imported" in str((b or {}).get("message") or ""):
            b = dict(b, message=None)
        # The same upload: live's new tab still reads as a plain file at the pick (doc-plus-import-png's image tab).
        if (step or {}).get("do") == "pick_files":
            ta, tb = (((a or {}).get("document") or {}).get("tabs") or []), (((b or {}).get("document") or {}).get("tabs") or [])
            for x, y in zip(ta, tb):
                if isinstance(x, dict) and isinstance(y, dict) and x.get("kind") == "file" and x.get("name") == y.get("name"):
                    y["kind"] = "file"
        # A side panel live read before it had loaded (its text still empty: face-rclick-assign-material's Material panel
        # filled by the next step) shows ours' loaded one: not compared there.
        ad = (a or {}).get("dialog")
        if isinstance(ad, dict) and isinstance(bd, dict) and ad.get("kind") == "panel" and bd.get("kind") == "panel" \
                and ad.get("title") == bd.get("title") and ad.get("text") == [] and bd.get("text"):
            b = dict(b, dialog=dict(bd, text=[]))
        # A toast shows from the action that raised it: after the step before's reading (ver-protection-enable: live's
        # was gone 3.4 s after the reading that first showed it, 7.5 s after the step before).
        if isinstance((a or {}).get("message"), str) and t is not None:
            toast_shown.setdefault(a["message"], prev_t if prev_t is not None else t)
        if a is not None and a.get("message") is None and isinstance((b or {}).get("message"), str) and t is not None \
                and t - toast_shown.get(b["message"], t) >= TOAST_MS:
            b = dict(b, message=None)
        # Live showing something else there (a dialog's notice): ours' toast hides what is under it, so the message
        # isn't compared (ver-protection-enable: the runner reopened the dialog before ours' toast timed out).
        elif a is not None and isinstance(a.get("message"), str) and isinstance((b or {}).get("message"), str) and a["message"] != b["message"] \
                and t is not None and t - toast_shown.get(b["message"], t) >= TOAST_MS:
            a, b = dict(a, message=None), dict(b, message=None)
        # A page (documents, preferences, help: live's state has its `url`) has no parts list: live's reader gives [], ours
        # null (prefs-units-learn-more).
        if isinstance(a, dict) and "url" in a and a.get("parts") == [] and isinstance(b, dict) and b.get("parts") is None:
            b = dict(b, parts=[])
        # A toast of ours raised at a step live's runner took longer than a toast lives (its 12 s steps: live 2477's
        # vs-cfg-value-unconfigure) was gone before live read the page: not compared there nor after, unless live shows it.
        if isinstance(a, dict) and isinstance((b or {}).get("message"), str) and t is not None:
            shown = a.get("message") if isinstance(a.get("message"), str) else ""
            slow = prev_t is not None and t - prev_t >= TOAST_MS
            kept = [m for m in b["message"].split(" / ") if m in shown or not ours_slow.setdefault(m, slow)]
            b = dict(b, message=" / ".join(kept) or None)
        if t is not None:
            prev_t = t
        # Live's runner on an assembly start's Part Studio (the lane's shared start, live 2418) still reads the lane's
        # assembly: no assembly where ours shows the Part Studio.
        if isinstance(a, dict) and a.get("assembly") is not None and a.get("partsHeading") is not None and (b or {}).get("assembly") is None \
                and (b or {}).get("partsHeading") is not None:
            a = dict(a, assembly=None)
        # (camera and measure: also once our side records them, the Part Studio's for now.)
        keys = KEYS + [k for k in LATER_KEYS if k in (a or {}) and (whole_document or k != "document") and (k not in ("camera", "measure") or k in (b or {}))]
        if (a or {}).get("drawing"):
            keys = [k for k in keys if k not in NOT_IN_DRAWINGS]
        a = {k: (a or {}).get(k) for k in keys}
        b = json.loads(json.dumps({k: (b or {}).get(k) for k in keys}))
        if str(live.get("recorded", "")) < PANEL_DIALOGS_SINCE and a.get("dialog") is None and is_panel_dialog(b.get("dialog")):
            b["dialog"] = None
        if str(live.get("recorded", "")) < POPOVER_PANELS_SINCE and a.get("dialog") is None and is_popover_panel(b.get("dialog")):
            b["dialog"] = None
        drop = PLAN_ROWS | LOCAL_ROWS | (CLIPBOARD_ROWS if clipboard_free else set())
        a, b = fold_start_edits(drop_rows(a, drop)), fold_start_edits(drop_rows(b, drop))
        b = drawn_rows_only(a, b)
        b = same_fieldless_focus(a, b)
        # A native <select> picked from: live's pointer stays on it and its title (the picked value) shows as a
        # tooltip; ours' pointer is on the list's row it picked (ver-filter-tab).
        pick = ((step or {}).get("on") or {}).get("pick") if isinstance(step, dict) else None
        if isinstance(pick, str) and a.get("tooltip") == pick and b.get("tooltip") is None:
            b["tooltip"] = pick
        if a.get("tooltip") is None and isinstance(b.get("tooltip"), str) and (tip_blanked or NATIVE_TITLE_TOOLTIPS.match(b["tooltip"])):
            b["tooltip"] = None
        # A page's native <select> holding the keyboard: its open list is the OS's popup, which live's recorder can't read.
        if isinstance(a.get("focus"), dict) and a["focus"].get("tag") == "select" and a.get("list") is None:
            b["list"] = None
        # A feature-style panel whose header live's recorder doesn't read (Insert Variable Studio: `title` null, so
        # its `titleState` and `ok` are defaults).
        if isinstance(a.get("dialog"), dict) and isinstance(b.get("dialog"), dict) and a["dialog"].get("kind") == "feature" and a["dialog"].get("title") is None:
            b["dialog"] = dict(b["dialog"], title=None, **{k: a["dialog"][k] for k in ("titleState", "ok") if k in a["dialog"]})
        # The Insert dialog's "Inserted: N": live's recorder (2026-11-14.1) puts it on the dialog's panels entry, ours
        # on the dialog (asm-insert-esc-pending / -ok-pending, where pending instances show only after ✓).
        for side in (a, b):
            dl = side.get("dialog")
            if isinstance(dl, dict) and dl.get("title") == "Insert parts and assemblies" and "inserted" not in dl:
                for pn in side.get("panels") or []:
                    if isinstance(pn, dict) and pn.get("id") == "assembly-insert-dialog" and "inserted" in pn:
                        dl["inserted"] = pn["inserted"]
        if isinstance(a.get("dialog"), dict) and isinstance(b.get("dialog"), dict) and ("inserted" in a["dialog"]) != ("inserted" in b["dialog"]):
            a["dialog"].pop("inserted", None)
            b["dialog"].pop("inserted", None)
        # A Feature Studio's notices while the autocomplete list is open (typing): live's count changes from run to run
        # at the same step (its compile runs on its own time), as ours.
        for side in (a, b):
            if isinstance(side.get("list"), dict) and side["list"].get("kind") == "autocomplete" and isinstance(side.get("featurestudio"), dict):
                side["featurestudio"].pop("notices", None)
        # A profile's times (Feature Studio Monitor › Profile: its badge, line bars and their tooltips) are the machine's.
        timed = lambda t: re.sub(r"\b\d+(\.\d+)? ?(ms|s)\b", "<time>", t) if isinstance(t, str) else t
        for side in (a, b):
            if isinstance(side.get("featurestudio"), dict):
                side["tooltip"] = timed(side.get("tooltip"))
                for pn in side.get("panels") or []:
                    if isinstance(pn, dict):
                        pn["text"] = timed(pn.get("text"))
        # Live lists a studio's child rows (Insert Variable Studio's variables) in a different order on each run.
        for side in (a, b):
            if isinstance(side.get("dialog"), dict) and isinstance(side["dialog"].get("fields"), list):
                side["dialog"]["fields"] = sorted_child_rows(side["dialog"]["fields"])
        # Create version's "Publish custom features and tables" shows when the workspace has a Feature Studio (live 2481):
        # live's lane document holds other runs' studios, so on a lane start live's checkbox is the lane's, not the
        # start's (ver-insert-create-version, ver-create-version-drawings-warning).
        publish = "Publish custom features and tables"
        ad, bd = a.get("dialog"), b.get("dialog")
        if not whole_document and isinstance(ad, dict) and isinstance(bd, dict) and ad.get("title") == bd.get("title") == "Create version from Main" \
                and any(isinstance(f, dict) and f.get("label") == publish for f in ad.get("fields") or []) \
                and not any(isinstance(f, dict) and f.get("label") == publish for f in bd.get("fields") or []):
            a["dialog"] = dict(ad, fields=[f for f in ad["fields"] if not (isinstance(f, dict) and f.get("label") == publish)])
        # A new subassembly is "Assembly <n>" with the document's next free number, which differs between live's lane
        # documents and ours ("Assembly 2 <1>" / "Assembly 1 <1>"): compared as one name.
        for side in (a, b):
            new_asm = lambda t: re.sub(r"\bAssembly \d+\b", "Assembly N", t) if isinstance(t, str) else t
            for r in ((side.get("features") or {}).get("rows") or []):
                r["name"] = new_asm(r.get("name"))
            lst = side.get("list")
            if isinstance(lst, dict):
                lst["rows"] = [new_asm(r) for r in lst.get("rows") or []]
            side["message"] = new_asm(side.get("message"))
            for i in ((side.get("assembly") or {}).get("instances") or []):
                i["name"] = new_asm(i.get("name"))
            for i in ((side.get("selection") or {}).get("items") or []):
                if isinstance(i, dict) and "owner" in i:
                    i["owner"] = new_asm(i["owner"])
        # Both runners name the start's assembly after it ("start pin-block asm Copy 2" / "start pin-block-asm-drawing asm").
        for side in (a, b):
            # (Its root row in the assembly's list too: drw-bom-switch-to-bom.)
            for r in ((side.get("features") or {}).get("rows") or []):
                if isinstance(r.get("name"), str):
                    r["name"] = re.sub(r"^start \S+ asm( Copy \d+)?$", "start asm", r["name"])
            d = side.get("dialog")
            if isinstance(d, dict) and d.get("kind") == "drawing-panel":
                # Live's DOM text keeps its no-break spaces ("Part\u00a01"); a document's name is each side's own. (First:
                # the names below are matched with spaces, "BOM\u00a0:\u00a0start\u00a0pin-block\u00a0asm", live2
                # drw-bom-table-properties.)
                d["text"] = [t.replace("\u00a0", " ") if isinstance(t, str) else t for t in d.get("text") or []]
                d["text"] = [re.sub(r"\bstart \S+ asm( Copy \d+)?\b", "start asm", t) if isinstance(t, str) else t for t in d["text"]]
                # Live's start studio is named after its start document ("start block Copy 1", live2 drw-custom-table-*).
                d["text"] = [re.sub(r"^start [\w-]+( Copy \d+)?$", "Part Studio 1", t) if isinstance(t, str) else t for t in d["text"]][:30]
                if d.get("title") in ("View properties", "BOM table properties", "Cut list table properties", "Custom table properties", "Image properties") and len(d["text"]) > 2 and d["text"][1] in ("Document", "Document:"):
                    d["text"][2] = "<document>"
            dr = side.get("drawing")
            if isinstance(dr, dict) and isinstance(dr.get("tab"), str):
                dr["tab"] = re.sub(r"^start \S+ asm( Copy \d+)? ", "start asm ", dr["tab"])
                # A drawing of live's start studio is named after it, as its dialogs' texts above ("start frame-drawing
                # Copy 1 Drawing 1", live2 drw-cut-list-table-properties).
                dr["tab"] = re.sub(r"^start (?!asm )[\w-]+( Copy \d+)? ", "Part Studio 1 ", dr["tab"])
                # A tab name live's lane document already had gets " (1)" (live2 drw-detail-view).
                dr["tab"] = re.sub(r" \(\d+\)$", "", dr["tab"])
        # Live's pixel reader lists what is drawn selected in its own scan order ([Top, Detail] for a detail view and its
        # boundary in the parent, drw-view-rotate-*): compared as a set.
        for side in (a, b):
            dr = side.get("drawing")
            if isinstance(dr, dict) and isinstance(dr.get("selection"), list):
                dr["selection"] = sorted(dr["selection"], key=lambda s: json.dumps(s, sort_keys=True))
        # Annotations alike in kind and text are listed in the drawing's order on live's side (its views' labels: Front,
        # Right, Top, Isometric), by position on ours: both sorted by kind, text and position.
        for side in (a, b):
            dr = side.get("drawing")
            if isinstance(dr, dict) and isinstance(dr.get("annotations"), list):
                dr["annotations"] = sorted(dr["annotations"], key=lambda x: (str(x.get("kind")), str(x.get("text")), [round(v) for v in (x.get("position") or [])]))
        # Live's recorder reads a table cell's text with its MText codes ("{\\pxql;AB}", live2 drw-table-cell-dblclick-type),
        # which it strips from notes: read as the plain text.
        for x in ((a.get("drawing") or {}).get("annotations") or []):
            if x.get("kind") == "table" and isinstance(x.get("text"), str):
                x["text"] = " ".join(re.sub(r"[{}]", "", re.sub(r"\\[A-Za-z][^;{}]*;", "", x["text"])).split())
        # Live's recorder can't read a table's cells (its text is null there).
        anns = [((side.get("drawing") or {}).get("annotations") or []) for side in (a, b)]
        if any(x.get("kind") == "table" and x.get("text") is None for x in anns[0]):
            for x in anns[1]:
                if x.get("kind") == "table":
                    x["text"] = None
        # Live's recorder lists an added sheet's format, its border and title block, as text-less lines (and the
        # projection symbol's circles) and the title block's notes ("DWG NO.", "----": 35 on an inserted sheet, 94 on
        # block-drawing's duplicate), not Sheet1's, though both sheets' are selectable item by item alike (live2
        # 2416): where live lists ten text-less lines or more and ours no line or note, live's lines, circles and
        # notes are left out.
        if sum(1 for x in anns[0] if x.get("kind") == "line" and x.get("text") is None) >= 10 and not any(x.get("kind") in ("line", "note") for x in anns[1]):
            dr = a.get("drawing")
            if isinstance(dr, dict):
                dr["annotations"] = [x for x in anns[0] if x.get("kind") not in ("line", "circle", "note")]
        # Live's tooltip text reads its line breaks as spaces ("…to fillet. 1 missing selection").
        for side in (a, b):
            if isinstance(side.get("tooltip"), str):
                side["tooltip"] = " ".join(side["tooltip"].split())
        # A configuration input's FeatureScript id is random ("List_WkWAMF6ZpKo2kz"): compared as one.
        for side in (a, b):
            d = side.get("dialog")
            if isinstance(d, dict):
                for f in d.get("fields") or []:
                    if isinstance(f, dict) and isinstance(f.get("value"), str):
                        f["value"] = re.sub(r"^List_[A-Za-z0-9]{14}$", "List_<id>", f["value"])
        # Live's account has folders, our run's scratch folder none: a location box's "No additional folders" notice
        # (ver-view-version-copy) is ours alone; and live's documents are named "live2 uip <start> <time>" where ours
        # are named after the start, so a "<document> - Copy" default compares as "<start> - Copy".
        if a.get("message") is None and b.get("message") == "No additional folders":
            b["message"] = None
        if isinstance(a.get("dialog"), dict):
            for f in a["dialog"].get("fields") or []:
                if isinstance(f, dict) and isinstance(f.get("value"), str):
                    f["value"] = re.sub(r"^live2? uip (\S+) \S+ - Copy$", r"\1 - Copy", f["value"])
        # Ours' in-app stand-in for the OS's file dialog (the runner has none): live's recorder can't read the OS's
        # (doc-plus-import, docs-create-import-files).
        if a.get("dialog") is None and isinstance(b.get("dialog"), dict) and b["dialog"].get("native"):
            b["dialog"], b["focus"] = None, a.get("focus")
        if isinstance(b.get("dialog"), dict):
            b["dialog"].pop("native", None)
        # The keyboard shortcuts sheet's search: live's reader labels it "Search shortcuts" in a Part Studio start and
        # by the sheet's header run together in a whole document's (shortcut-shift-slash / doc-shortcuts-search).
        for side in (a, b):
            for f in ((side.get("dialog") or {}).get("fields") or []) if isinstance(side.get("dialog"), dict) else []:
                if isinstance(f, dict) and f.get("label") == "Keyboard shortcutsCustomize keyboard shortcuts":
                    f["label"] = "Search shortcuts"
        # A paid plan's upsell tooltip (docs-account-my-account: "Try Onshape Professional"): ours has no plans.
        if isinstance(a.get("tooltip"), str) and "Onshape Professional" in a["tooltip"] and b.get("tooltip") is None:
            b["tooltip"] = a["tooltip"]
        # A user's name (live's account's, ours the system's): Drawn by (doc-tab-properties-drawing).
        for side in (a, b):
            for f in ((side.get("dialog") or {}).get("fields") or []) if isinstance(side.get("dialog"), dict) else []:
                if isinstance(f, dict) and f.get("label") == "Drawn by" and f.get("value"):
                    f["value"] = "<user>"
                # A date the day sets (Date drawn, Last changed date: today's on each side) as a date, not the literal.
                if isinstance(f, dict) and "date" in str(f.get("label") or "").lower() and isinstance(f.get("value"), str) and re.fullmatch(r"\d{2}/\d{2}/\d{4}", f["value"]):
                    f["value"] = "<MM/DD/YYYY>"
        # The Free plan's public-document icon's tooltip under the pointer (docs-row-menu): ours has no public documents.
        if a.get("tooltip") == "This is a public document." and b.get("tooltip") is None:
            b["tooltip"] = a["tooltip"]
        # Live's Free plan answers a paid feature (Release…, vs-tab-release) with its upgrade modal where ours opens the
        # feature's dialog: a plan's limit, not compared (as PLAN_ROWS).
        if isinstance(a.get("dialog"), dict) and "Start Professional Trial" in (a["dialog"].get("buttons") or []):
            b["dialog"], b["focus"] = a["dialog"], a.get("focus")
        drawn_sizes(a, b)
        placed_alike(a, b)
        cloud_menu(a, b)
        d = first_difference(a, b)
        if d is None:
            matched += 1
        elif first is None:
            first = (name,) + d
        for path, x, y in all_differences(a, b):
            # Causes: the key path without row indices, with one example.
            key = re.sub(r"\[\d+\]", "[]", path)
            CAUSES.setdefault(key, [0, f"{json.dumps(x, ensure_ascii=False)} / {json.dumps(y, ensure_ascii=False)}"])[0] += 1
    return matched, len(pairs), first


def check_expect(scenario, ours):
    """qa's `expect` entries (live facts it knows) against our recording: the failed ones, as text."""
    steps = {s.get("step"): s.get("state") or {} for s in ours.get("steps", [])}
    failed = []
    for e in scenario.get("expect", []):
        st = steps.get(e.get("after"))
        if st is None:
            continue
        focus, dialog, lst = st.get("focus") or {}, st.get("dialog") or {}, st.get("list") or {}
        checks = []
        if "tooltip" in e:
            checks.append(("tooltip", st.get("tooltip") == e["tooltip"], st.get("tooltip")))
        if "selection" in e:
            sel = focus.get("selection") or [None, None]
            ok = focus.get("label") == e.get("field") and (focus.get("all") if e["selection"] == "all" else sel[0] == sel[1])
            checks.append(("selection", ok, focus))
        elif "text" in e and "field" in e:
            f = next((f for f in dialog.get("fields", []) if f.get("label") == e["field"]), {})
            checks.append(("text", f.get("text") == e["text"], f.get("text")))
        if "list" in e:
            checks.append(("list", lst.get("rows") == e["list"] and lst.get("highlighted") == e.get("highlighted", lst.get("highlighted")), lst))
        if "popup" in e:
            name = next((f.get("text") for f in dialog.get("fields", []) if f.get("label") == "Name"), None)
            checks.append(("popup", dialog.get("title") == e["popup"] and name == e.get("name", name), dialog.get("title")))
        failed += [f"after {e.get('after')} {what}: ours {json.dumps(got, ensure_ascii=False)[:120]}" for what, ok, got in checks if not ok]
    return failed


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("names", nargs="*")
    ap.add_argument("--app", default=os.path.join(os.environ.get("CARGO_TARGET_DIR", str(ROOT / "target")), "debug", "noshape"))
    ap.add_argument("--scenarios", default=str(ROOT / "tests" / "ui-parity"))
    ap.add_argument("--live", default=None, help="live recordings (default: <scenarios>/live)")
    ap.add_argument("--out", default=os.path.join(tempfile.gettempdir(), "noshape-ui-parity"))
    ap.add_argument("--display", default=os.environ.get("DISPLAY", ""))
    ap.add_argument("--timeout", type=float, default=240)
    ap.add_argument("--all", action="store_true", help="every scenario, live recording or not")
    ap.add_argument("--compare-only", action="store_true")
    args = ap.parse_args()
    scen_dir, out_dir = Path(args.scenarios), Path(args.out)
    live_dir = Path(args.live) if args.live else scen_dir / "live"
    global STARTS
    STARTS = json.loads((scen_dir / "starts.json").read_text())
    out_dir.mkdir(parents=True, exist_ok=True)
    names = args.names or sorted(p.stem for p in scen_dir.glob("*.json") if p.stem != "starts" and (args.all or (live_dir / p.name).exists()))
    if not names:
        print(f"no scenarios with a live recording in {live_dir} (--all runs them anyway)")
        return 1
    score, steps_ok, steps_all = 0, 0, 0
    for name in names:
        out = out_dir / f"{name}.json"
        if not (scen_dir / f"{name}.json").exists():
            print(f"{name}: no scenario file")
            continue
        if not args.compare_only:
            problem = run_one(args.app, scen_dir / f"{name}.json", out, args.display, args.timeout)
            # The display refusing a new client now and then (Xvfb): once more.
            if problem and "platform.rs" in problem:
                time.sleep(2)
                problem = run_one(args.app, scen_dir / f"{name}.json", out, args.display, args.timeout)
            if problem:
                print(f"{name}: {problem}")
                continue
        live = live_dir / f"{name}.json"
        if not out.exists():
            print(f"{name}: not run")
            continue
        if not live.exists():
            try:
                failed = check_expect(json.loads((scen_dir / f"{name}.json").read_text()), json.loads(out.read_text()))
            except (OSError, ValueError) as e:
                failed = [str(e)]
            print(f"{name}: recorded, no live recording" + (f"; expect FAILED: {'; '.join(failed)}" if failed else ""))
            continue
        try:
            ours = json.loads(out.read_text())
        except (OSError, ValueError) as e:
            print(f"{name}: {e}")
            continue
        start = json.loads((scen_dir / f"{name}.json").read_text()).get("start")
        whole = isinstance(start, str) and any(k in STARTS.get(start, {}) for k in ("tabs", "page"))
        m, n, first = compare(forget_old_query_states(json.loads(live.read_text())), ours, whole)
        steps_ok, steps_all = steps_ok + m, steps_all + n
        if first is None:
            score += 1
            print(f"{name}: match ({n} states)")
        else:
            where, path, a, b = first
            print(f"{name}: {where} differs at {path}: live {json.dumps(a, ensure_ascii=False)} / ours {json.dumps(b, ensure_ascii=False)} ({m}/{n} states)")
    print(f"score: {score}/{len(names)} scenarios, {steps_ok}/{steps_all} states")
    if CAUSES:
        print("differences by key (states, live / ours example):")
        for key, (n, ex) in sorted(CAUSES.items(), key=lambda kv: -kv[1][0])[:15]:
            print(f"  {n:4d}  {key}: {ex[:140]}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
