#!/usr/bin/env python3
"""Guard: user content is never translated.

UI text is translated where it's drawn, and user-authored text (document,
tab, part, feature, variable… names) renders through `ui::user_text`, which
the lookup skips. This builds a document whose parts, features, tabs and
variables are named after UI strings ("Extrude", "Sketch", "Top", "Part 1",
"Mate connector"), opens it in the pseudo-locale (every looked-up text
becomes ⟦text⟧) with the text log on, walks the feature list, parts list,
tabs, a feature dialog's selection chips, the BOM and the Variable Studio,
and fails if any of those names was looked up there, or wasn't drawn as user
text.

usage: scripts/qa-i18n-user-text.py [--keep]
  needs Xvfb and xdotool; NOSHAPE_BIN / NOSHAPE_CLI (default
  target/debug/noshape, target/debug/noshape-cli), DISPLAY (default: starts
  Xvfb :117).
"""

import json
import os
import shutil
import subprocess
import sys
import tempfile
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
TARGET = os.environ.get("CARGO_TARGET_DIR", os.path.join(ROOT, "target"))
BIN = os.environ.get("NOSHAPE_BIN", os.path.join(TARGET, "debug", "noshape"))
CLI = os.environ.get("NOSHAPE_CLI", os.path.join(TARGET, "debug", "noshape-cli"))
W, H = 1920, 1080


def cli(doc, op, **args):
    if op != "document_create":
        args.setdefault("doc", doc)
    r = subprocess.run([CLI, "call", op, json.dumps(args)], capture_output=True, text=True)
    if r.returncode:
        sys.exit(f"{op}: {r.stdout}{r.stderr}")
    return json.loads(r.stdout)


def make_document(doc):
    """Part Studio "Extrude": sketch "Extrude", extrude "Sketch" (its part
    "Part 1", part number "Extrude", description "Sketch"), transform
    "Mate connector" of that part, variable #Top;
    Variable Studio "Sketch" with #Top described "Extrude"; Assembly "Top"
    with Part 1 inserted."""
    cli(doc, "document_create", path=doc)
    cli(doc, "sketch_create", plane="plane:top", name="Extrude")
    regions = cli(doc, "sketch_draw", sketch="Extrude", tool="corner_rectangle", points=[[0, 0], [40, 20]])["regions"]
    cli(doc, "feature_add", type="extrude", name="Sketch", params={"entities": [r["id"] for r in regions], "depth": "10 mm"})
    body = cli(doc, "query_parts")["parts"][0]["id"]
    cli(doc, "part_set_properties", part=body, part_number="Extrude", description="Sketch")
    cli(doc, "feature_add", type="transform", name="Mate connector", params={"entities": [body], "transformType": "TRANSLATION_3D", "dx": "5 mm"})
    cli(doc, "variable_create", name="Top", value="5 mm")
    cli(doc, "tab_create", kind="variablestudio", name="Sketch")
    cli(doc, "variable_create", tab="Sketch", name="Top", value="5 mm", description="Extrude")
    cli(doc, "tab_rename", tab="Part Studio 1", name="Extrude")
    cli(doc, "tab_rename", tab="Assembly 1", name="Top")
    cli(doc, "assembly_insert", tab="Top", source="Extrude", part=body)


class Log:
    """The app's text log: `x y w h T|U text` per painted text and place."""

    def __init__(self, path):
        self.path, self.pos = path, 0

    def lines(self):
        out = []
        with open(self.path, encoding="utf-8") as f:
            for line in f:
                x, y, w, h, kind, text = line.rstrip("\n").split("\t", 5)
                out.append((float(x), float(y), float(w), float(h), kind, text))
        return out

    def mark(self):
        self.pos = len(self.lines())

    def since(self):
        return self.lines()[self.pos:]


def xdo(*args):
    subprocess.run(["xdotool", *map(str, args)], check=True)


def click(x, y, double=False):
    xdo("mousemove", int(x), int(y))
    time.sleep(0.2)
    if double:
        xdo("click", "--repeat", 2, "--delay", 1, 1)
    else:
        xdo("click", 1)


def find(lines, text, kind="U", where=lambda x, y: True):
    for x, y, w, h, k, t in reversed(lines):
        if t == text and k == kind and where(x, y):
            return x + w / 2, y + h / 2
    return None


def main():
    keep = "--keep" in sys.argv
    work = tempfile.mkdtemp(prefix="noshape-i18n-guard-")
    doc = os.path.join(work, "guard.noshape")
    make_document(doc)

    xvfb = None
    env = dict(os.environ)
    if "DISPLAY" not in env:
        env["DISPLAY"] = ":117"
        xvfb = subprocess.Popen(["Xvfb", ":117", "-screen", "0", f"{W}x{H}x24"], stderr=subprocess.DEVNULL)
        time.sleep(1)
    os.environ["DISPLAY"] = env["DISPLAY"]
    log_path = os.path.join(work, "text.log")
    env.update(HOME=os.path.join(work, "home"), NOSHAPE_NO_NATIVE_DIALOGS="1", NOSHAPE_PSEUDO_LOCALE="1", NOSHAPE_I18N_LOG=log_path)
    os.makedirs(env["HOME"])
    app = subprocess.Popen([BIN, doc], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    failures = []
    try:
        for _ in range(60):
            r = subprocess.run(["xdotool", "search", "--pid", str(app.pid), "--name", "noshape"], capture_output=True, text=True)
            if r.stdout.strip():
                break
            time.sleep(0.5)
        win = r.stdout.split()[0]
        xdo("windowmove", win, 0, 0)
        xdo("windowsize", win, W, H)
        time.sleep(8)
        log = Log(log_path)

        def expect(step, names, lines=None):
            lines = log.since() if lines is None else lines
            drawn = {t for *_, k, t in lines if k == "U"}
            for n in names:
                if n not in drawn:
                    failures.append(f"{step}: \"{n}\" wasn't drawn as user text")

        # 1. The Part Studio: feature list, parts list, tabs.
        expect("Part Studio", ["Extrude", "Sketch", "Mate connector", "Part 1", "Top"], log.lines())
        # 2. Edit the transform: its selection chip shows the part's name.
        row = find(log.lines(), "Mate connector", where=lambda x, y: x < 400)
        if row is None:
            failures.append("the feature row \"Mate connector\" wasn't found")
        else:
            log.mark()
            click(*row, double=True)
            time.sleep(3)
            expect("Transform dialog", ["Part 1"])
            xdo("key", "Escape")
            time.sleep(1)
        # 3. The Variable Studio tab "Sketch": its title, and #Top described
        # "Extrude" (table cells are inputs, which are never looked up; the
        # check below covers them).
        tab = find(log.lines(), "Sketch", where=lambda x, y: y > H - 60)
        if tab is None:
            failures.append("the tab \"Sketch\" wasn't found")
        else:
            log.mark()
            click(*tab)
            time.sleep(3)
            expect("Variable Studio", ["Sketch"])
        # 4. The Assembly tab "Top": its instance list, and the BOM's part
        # number and description.
        tab = find(log.lines(), "Top", where=lambda x, y: y > H - 60)
        if tab is None:
            failures.append("the tab \"Top\" wasn't found")
        else:
            log.mark()
            click(*tab)
            time.sleep(4)
            expect("Assembly", ["Top", "Origin", "Part 1 <1>"])
            # The BOM's button is in the right-hand strip: hover each until
            # its tooltip shows.
            bom = None
            for y in range(440, 700, 8):
                xdo("mousemove", W - 17, y)
                time.sleep(0.9)
                if find(log.lines(), "⟦BOM Table⟧", kind="T"):
                    bom = (W - 17, y)
                    break
            if bom is None:
                failures.append("the BOM button wasn't found")
            else:
                log.mark()
                click(*bom)
                time.sleep(3)
                expect("BOM", ["Extrude", "Sketch"])
        shot = os.path.join(work, "last.png")
        subprocess.run(["import", "-window", "root", shot], env=env)

        # Never looked up: no user name shows as ⟦name⟧ anywhere but in the
        # toolbar's "Sketch" button (a UI string).
        allowed = {("Sketch", "toolbar")}
        for x, y, w, h, k, t in log.lines():
            if k != "T" or not (t.startswith("⟦") and t.endswith("⟧")):
                continue
            name = t[1:-1]
            if name not in {"Extrude", "Sketch", "Top", "Part 1", "Mate connector"}:
                continue
            place = "toolbar" if y < 110 else "elsewhere"
            if (name, place) not in allowed:
                failures.append(f"\"{name}\" was looked up at ({x:.0f}, {y:.0f})")
    finally:
        app.terminate()
        app.wait()
        if xvfb:
            xvfb.terminate()

    if failures:
        print("FAIL\n  " + "\n  ".join(dict.fromkeys(failures)))
        print(f"  log: {log_path}")
        sys.exit(1)
    print("ok: user names are never translated")
    if not keep:
        shutil.rmtree(work, ignore_errors=True)


if __name__ == "__main__":
    main()
