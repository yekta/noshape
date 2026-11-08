"""The documents page's folder links on a real desktop (qa's release check, qa-os.yml): the footer's path opens the
documents folder in Explorer / Finder, a row's "Show in Explorer / Finder" shows the document there, selected, and a
screenshot shows the footer and Details › Location as the OS writes the path. Driven by the app's UI script
(NOSHAPE_UI_SCRIPT, tests/ui-parity/README.md) with the docs-page start in the user's own documents folder.

  python footer.py --bin <noshape> --out <dir>
"""
import argparse
import json
import os
import subprocess
import sys
import time
from pathlib import Path

a = argparse.ArgumentParser()
a.add_argument("--bin", required=True)
a.add_argument("--out", required=True)
args = a.parse_args()
out = Path(args.out)
out.mkdir(parents=True, exist_ok=True)
WIN = sys.platform == "win32"
docs = Path.home() / "Documents" / "noshape"
label = "Show in Explorer" if WIN else "Show in Finder"
starts = Path("tests/ui-parity/starts.json").resolve()
problems = []


def say(m):
    print(m, flush=True)


def scenario(name, steps, hold_ms):
    p = out / f"{name}.json"
    p.write_text(json.dumps({"name": name, "about": "qa-os", "area": "studios", "start": "docs-page", "steps": steps + [{"do": "wait", "ms": hold_ms}]}))
    return p


def run(name, steps, hold_ms=4000, shot_after=None):
    env = dict(os.environ, NOSHAPE_UI_SCRIPT=str(scenario(name, steps, hold_ms)), NOSHAPE_UI_STATE_OUT=str(out / f"{name}-state.json"),
               NOSHAPE_UI_STARTS=str(starts), NOSHAPE_CONFIG_DIR=str(out / f"cfg-{name}"), NOSHAPE_DOCUMENTS_DIR=str(docs))
    with open(out / f"{name}-app.log", "w") as log:
        p = subprocess.Popen([args.bin], env=env, stdout=log, stderr=log)
        if shot_after:
            time.sleep(shot_after)
            from PIL import ImageGrab
            ImageGrab.grab().save(out / f"{name}.png")
        try:
            p.wait(timeout=150)
        except subprocess.TimeoutExpired:
            p.kill()
            problems.append(f"{name}: the app didn't finish the script in 150 s")
    state = json.loads((out / f"{name}-state.json").read_text()) if (out / f"{name}-state.json").exists() else {"error": "no state written"}
    if state.get("error"):
        problems.append(f"{name}: {state['error']}")
    for s in state.get("steps", []):
        if not s.get("ok", True):
            problems.append(f"{name}: step {s.get('step')} failed: {json.dumps(s.get('do'))} {s.get('error', '')}")
    return state


def windows():
    """The file manager's open windows: [(folder, [selected names])]."""
    if WIN:
        ps = ("$r=@(); foreach($w in (New-Object -ComObject Shell.Application).Windows()){ try { $f=$w.Document.Folder.Self.Path;"
              " $s=@($w.Document.SelectedItems() | % { $_.Name }); $r+= ,@{f=$f; s=$s} } catch {} }; ConvertTo-Json -Depth 4 -InputObject $r")
        txt = subprocess.run(["powershell", "-NoProfile", "-Command", ps], capture_output=True, text=True, timeout=60).stdout.strip() or "[]"
        v = json.loads(txt)
        v = v if isinstance(v, list) else [v]
        return [(w["f"], w["s"] if isinstance(w["s"], list) else [w["s"]]) for w in v if w]
    # Names, not paths: Finder won't hand a script a path inside ~/Documents without the runner's Documents permission.
    osa = ('set r to ""\ntell application "Finder"\nrepeat with k from 1 to (count of Finder windows)\ntry\nset r to r & "W " & (name of Finder window k) & linefeed\nend try\nend repeat\n'
           'try\nrepeat with i in (get selection)\nset r to r & "SEL " & (name of i) & linefeed\nend repeat\nend try\nend tell\nreturn r')
    res = subprocess.run(["osascript", "-e", osa], capture_output=True, text=True, timeout=60)
    if res.returncode:
        say(f"osascript: {res.stderr.strip()}")
    lines = [l for l in res.stdout.splitlines() if l.strip()]
    sel = [l[4:] for l in lines if l.startswith("SEL ")]
    return [(l[2:], sel) for l in lines if l.startswith("W ")]


def close_windows():
    if WIN:
        subprocess.run(["powershell", "-NoProfile", "-Command", "(New-Object -ComObject Shell.Application).Windows() | % { $_.Quit() }"], timeout=60)
    else:
        subprocess.run(["osascript", "-e", 'tell application "Finder" to close every window'], timeout=60)
    time.sleep(2)


def same(a_, b_):
    if not WIN:
        return str(a_) == Path(b_).name
    return os.path.normcase(os.path.normpath(str(a_).rstrip("/\\"))) == os.path.normcase(os.path.normpath(str(b_)))


close_windows()
say(f"documents folder {docs}; menu item {label!r}")
run("footer-link", [{"do": "click", "on": {"button": "documents-folder"}}])
w = windows()
say(f"after the footer link: {w}")
if not any(same(f, docs) for f, _ in w):
    problems.append(f"the footer link opened no window on {docs} (windows: {w})")
close_windows()

st = run("show-in", [{"do": "rclick", "on": {"doc_row": "uip Bracket"}}, {"do": "click", "on": {"menu": label}}])
rows = next((s["state"]["list"]["rows"] for s in st.get("steps", []) if (s.get("state") or {}).get("list")), [])
say(f"row menu: {rows}")
w = windows()
say(f"after {label}: {w}")
if not any(same(f, docs) and "uip Bracket" in [Path(x).name.removesuffix(".noshape") for x in s] for f, s in w):
    problems.append(f"{label} didn't show uip Bracket.noshape selected in {docs} (windows: {w})")
close_windows()

run("details", [{"do": "rclick", "on": {"doc_row": "uip Bracket"}}, {"do": "click", "on": {"menu": "Details"}}], hold_ms=12000, shot_after=14)


def real_rclick(item):
    """Windows: a real right-click (mouse_event) on uip Bracket's row (the UI script's rclick opens no menu there), then the
    script clicks the menu's item."""
    import ctypes
    from ctypes import wintypes
    u = ctypes.windll.user32
    name = "real-" + item.split()[0].lower()
    env = dict(os.environ, NOSHAPE_UI_SCRIPT=str(scenario(name, [{"do": "wait", "ms": 14000}, {"do": "click", "on": {"menu": item}}], 6000 if item == label else 3000)), NOSHAPE_UI_STATE_OUT=str(out / f"{name}-state.json"),
               NOSHAPE_UI_STARTS=str(starts), NOSHAPE_CONFIG_DIR=str(out / f"cfg-{name}"), NOSHAPE_DOCUMENTS_DIR=str(docs))
    with open(out / f"{name}-app.log", "w") as log:
        p = subprocess.Popen([args.bin], env=env, stdout=log, stderr=log)
        time.sleep(8)
        found = []
        cb = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)
        def each(h, _):
            pid = wintypes.DWORD()
            u.GetWindowThreadProcessId(h, ctypes.byref(pid))
            if pid.value == p.pid and u.IsWindowVisible(h):
                found.append(h)
            return True
        u.EnumWindows(cb(each), 0)
        if not found:
            problems.append(f"{name}: no window of the app found")
        else:
            h = found[0]
            u.SetForegroundWindow(h)
            pt = wintypes.POINT(0, 0)
            u.ClientToScreen(h, ctypes.byref(pt))
            x, y = pt.x + 359, pt.y + 322  # uip Bracket's name on a 1024 x 768 runner screen (qa-os details.png)
            say(f"{name}: right-click at {x}, {y} (client origin {pt.x}, {pt.y})")
            u.SetCursorPos(x, y)
            time.sleep(0.3)
            u.mouse_event(0x0008, 0, 0, 0, 0)  # RIGHTDOWN
            time.sleep(0.05)
            u.mouse_event(0x0010, 0, 0, 0, 0)  # RIGHTUP
            time.sleep(2)
            from PIL import ImageGrab
            ImageGrab.grab().save(out / f"{name}.png")
        try:
            p.wait(timeout=60)
        except subprocess.TimeoutExpired:
            p.kill()
    st = json.loads((out / f"{name}-state.json").read_text()) if (out / f"{name}-state.json").exists() else {}
    lists = [s["state"]["list"] for s in st.get("steps", []) if (s.get("state") or {}).get("list")]
    say(f"{name}: menus seen after the click: {lists}")
    if not lists:
        problems.append(f"{name}: a real right-click on uip Bracket's row opened no menu (screenshot {name}.png)")
    for s_ in st.get("steps", []):
        if not s_.get("ok", True):
            problems.append(f"{name}: step {s_.get('step')} failed: {json.dumps(s_.get('do'))} {s_.get('error', '')}")


if WIN:
    close_windows()
    real_rclick(label)
    w = windows()
    say(f"after a real right-click and {label}: {w}")
    if any(same(f, docs) and "uip Bracket" in [Path(x).name.removesuffix(".noshape") for x in s] for f, s in w):
        problems[:] = [p_ for p_ in problems if not p_.startswith(("show-in:", f"{label} didn't"))]
        say(f"{label} works from a real right-click: the UI script's rclick is what fails on Windows")
    else:
        problems.append(f"{label} after a real right-click didn't show uip Bracket.noshape selected (windows: {w})")
    close_windows()

text = "\n".join(f"- {p}" for p in problems) or "- the footer link opened the documents folder; Show in opened its folder with the document selected"
say(text)
if os.environ.get("GITHUB_STEP_SUMMARY"):
    with open(os.environ["GITHUB_STEP_SUMMARY"], "a", encoding="utf-8") as f:
        f.write(f"### {'❌' if problems else '✅'} Documents page folder links ({sys.platform})\n\n{text}\n\n")
sys.exit(1 if problems else 0)
