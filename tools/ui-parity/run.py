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
LATER_KEYS = ["undo", "sketch", "assembly", "drawing", "document", "documents", "preferences", "variables", "featurestudio"]
# Read from the main page, which a drawing's iframe leaves alone (README "### Drawings"): not compared there.
NOT_IN_DRAWINGS = {"selection", "undo", "assembly"}


def remembered_settings(seed):
    """A scenario's "remembered" (what live's account last committed per feature type, behaviours 1987) as the app's
    feature_settings.json: type → group ("", "length:Millimeter", "angle:Degree") → parameter → value. Plain values:
    a list is a Lookup path, a bool a Bool, a string in a length / angle group an expression, else an Enum."""
    def value(group, v):
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
        return "no recording: " + " | ".join(l for l in err.splitlines() if "UI-SCRIPT" in l or "panicked" in l)[:300]
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


def first_difference(a, b, path=""):
    """The first key path where `a` (live) and `b` (ours) differ, with both values. Keys only one side records
    (a side's debugging aids) are not compared; the selection compares its count only until both record kind and owner."""
    if isinstance(a, dict) and isinstance(b, dict):
        for k in a:
            if k not in b:
                continue
            # Live's recorder couldn't read the sketch (`"error": "sketch model not reached"`, before a plane is picked).
            if path == ".sketch" and a.get("error") and k in ("entities", "constraints", "dimensions"):
                continue
            if path == ".selection" and k == "items" and not all("owner" in i and i.get("kind") for i in (a[k] or [])):
                continue
            # A flip button has no label of its own (live records its icon's name).
            if k == "label" and a.get("kind") == "toggle":
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
            out += all_differences(a[k], b[k], f"{path}.{k}")
        return out
    if isinstance(a, list) and isinstance(b, list):
        out = [d for i, (x, y) in enumerate(zip(a, b)) for d in all_differences(x, y, f"{path}[{i}]")]
        return out + ([(f"{path}.length", len(a), len(b))] if len(a) != len(b) else [])
    return [] if a == b else [(path, a, b)]


# Cloud features noshape leaves out (AGENTS.md: comments, sharing…): not compared in live's menus.
CLOUD_ONLY = {"Add comment", "Copy link", "Share…", "Transfer ownership…", "Applications", "Create CAM Studio", "Create PCB Studio",
              "Create Render Studio", "Create tutorial…", "Explore Onshape", "What's new", "Forums", "Report a bug",
              "Import from"}


def normalize_live(state):
    """Live's state in the agreed form: cloud-only menu rows dropped, no highlighted row as null (live: -1)."""
    # A focused checkbox: the DOM gives its value "on" and no selection; it has no text.
    focus = (state or {}).get("focus")
    if isinstance(focus, dict) and focus.get("text") == "on" and focus.get("selection") == [None, None]:
        focus["text"], focus["selection"] = None, None
    # Live's recorder leaves an instance's owner empty when a command selected it (Select instances with same
    # configuration, Dynamic suppression's panel), not a click: compared by kind.
    sel = (state or {}).get("selection")
    if isinstance(sel, dict):
        for it in sel.get("items") or []:
            if isinstance(it, dict) and it.get("kind") == "INSTANCE" and it.get("owner") is None and it.get("id") != "assemblyRoot":
                it.pop("owner", None)
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
    if isinstance(d, dict) and d.get("kind") == "modal":
        for f in d.get("fields") or []:
            if isinstance(f, dict) and str(f.get("label", "")).startswith("File name") and isinstance(f.get("value"), str):
                f["value"] = re.sub(r"^start \S+( Copy \d+)? - ", "Part Studio 1 - ", f["value"])
    # A REST-built start's history reads "Add feature" where the UI's (ours) reads "Insert feature" (live2 2258).
    doc = (state or {}).get("document")
    if isinstance(doc, dict):
        for h in doc.get("history") or []:
            if isinstance(h, dict) and isinstance(h.get("name"), str):
                h["name"] = h["name"].replace(" :: Add feature : ", " :: Insert feature : ")
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
        lst["rows"] = [r for r in rows if r not in CLOUD_ONLY]
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
        lst["highlighted"] = lst["rows"].index(hl_row) if hl_row in lst["rows"] else None
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
CLIPBOARD_ROWS = {"Paste into sketch", "Paste sketch", "Paste sketch entities"}


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
PLAN_ROWS = {"Thickness analysis…", "Edit thickness analysis", "Turn thickness analysis off"}


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


def drop_rows(state, drop):
    lst = (state or {}).get("list")
    if not isinstance(lst, dict) or lst.get("kind") != "context-menu":
        return state
    rows = lst.get("rows") or []
    hl = lst.get("highlighted")
    hl_row = rows[hl] if isinstance(hl, int) and 0 <= hl < len(rows) else None
    lst["rows"] = [r for r in rows if r not in drop]
    lst["highlighted"] = lst["rows"].index(hl_row) if hl_row in lst["rows"] else None
    return state


def compare(live, ours, whole_document=True):
    """(matched steps, total steps, first difference or None). `whole_document`: the scenario starts on a whole
    document or a page (README "Documents, tabs and preferences"); otherwise live's runner works in a lane copy whose
    tab bar isn't the start's, so `document` isn't compared."""
    pairs = [("start", live.get("start"), ours.get("start"), None)]
    clipboard_free = not copies(live)
    ours_steps = {s.get("step"): s for s in ours.get("steps", [])}
    for s in live.get("steps", []):
        o = ours_steps.get(s.get("step"), {})
        # A step ours couldn't do (its target not found) while live did: a difference, whatever the state after.
        failed = o.get("error") if o.get("ok") is False and s.get("ok") is not False else None
        pairs.append((f"step {s.get('step')}", s.get("state"), o.get("state"), failed))
    matched, first = 0, None
    for name, a, b, failed in pairs:
        if failed:
            if first is None:
                first = (name, ".step", "ok", f"failed: {failed}")
            CAUSES.setdefault(".step", [0, f"ok / failed: {failed}"])[0] += 1
            continue
        a = normalize_live(json.loads(json.dumps(a)))
        keys = KEYS + [k for k in LATER_KEYS if k in (a or {}) and (whole_document or k != "document")]
        if (a or {}).get("drawing"):
            keys = [k for k in keys if k not in NOT_IN_DRAWINGS]
        a = {k: (a or {}).get(k) for k in keys}
        b = json.loads(json.dumps({k: (b or {}).get(k) for k in keys}))
        if str(live.get("recorded", "")) < PANEL_DIALOGS_SINCE and a.get("dialog") is None and is_panel_dialog(b.get("dialog")):
            b["dialog"] = None
        drop = PLAN_ROWS | LOCAL_ROWS | (CLIPBOARD_ROWS if clipboard_free else set())
        a, b = fold_start_edits(drop_rows(a, drop)), fold_start_edits(drop_rows(b, drop))
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
            d = side.get("dialog")
            if isinstance(d, dict) and d.get("kind") == "drawing-panel":
                d["text"] = [re.sub(r"\bstart \S+ asm( Copy \d+)?\b", "start asm", t) if isinstance(t, str) else t for t in d.get("text") or []]
                # Live's start studio is named after its start document ("start block Copy 1", live2 drw-custom-table-*).
                d["text"] = [re.sub(r"^start [\w-]+( Copy \d+)?$", "Part Studio 1", t) if isinstance(t, str) else t for t in d["text"]]
                # Live's DOM text keeps its no-break spaces ("Part\u00a01"); a document's name is each side's own.
                d["text"] = [t.replace("\u00a0", " ") if isinstance(t, str) else t for t in d["text"]][:30]
                if d.get("title") in ("View properties", "BOM table properties", "Cut list table properties", "Image properties") and len(d["text"]) > 2 and d["text"][1] in ("Document", "Document:"):
                    d["text"][2] = "<document>"
            dr = side.get("drawing")
            if isinstance(dr, dict) and isinstance(dr.get("tab"), str):
                dr["tab"] = re.sub(r"^start \S+ asm( Copy \d+)? ", "start asm ", dr["tab"])
                # A tab name live's lane document already had gets " (1)" (live2 drw-detail-view).
                dr["tab"] = re.sub(r" \(\d+\)$", "", dr["tab"])
        # Live's recorder can't read a table's cells (its text is null there).
        anns = [((side.get("drawing") or {}).get("annotations") or []) for side in (a, b)]
        if any(x.get("kind") == "table" and x.get("text") is None for x in anns[0]):
            for x in anns[1]:
                if x.get("kind") == "table":
                    x["text"] = None
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
