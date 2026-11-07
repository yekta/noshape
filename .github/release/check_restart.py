"""A previous release, installed as a user installs it, updates itself to the newest release on GitHub through its
own update card (release.yml's after-publish job; no test hook: a release build only reads GitHub's releases/latest
and only restarts when Restart is pressed). Started with a document, it must find, download and verify the release;
the card's Restart button (found on the screen by its colour) is clicked; then the new version must run with the
document reopened and be the one installed. Watched through the app's log (NOSHAPE_UPDATE_LOG, read by every build).

  python check_restart.py --want <version> --exe <installed program> --log <log> --doc <document> --shots <dir> -- <start command...>

The start command starts the installed app with the document, as a user does (the log and NOSHAPE_NO_SETUP_PROMPT are
in its environment). GitHub allows 60 unauthenticated requests an hour per address and the runners share addresses:
a rate-limited app waits as long as GitHub says (up to an hour), so the wait for the release is long. Exits 1 with
the reason (also in the job summary) on any failure.
"""
import argparse
import ctypes
import json
import os
import subprocess
import sys
import time

from PIL import ImageGrab

a = argparse.ArgumentParser()
a.add_argument("--want", required=True)
a.add_argument("--exe", required=True)
a.add_argument("--log", required=True)
a.add_argument("--doc", required=True)
a.add_argument("--shots", required=True)
a.add_argument("--label", default="")
a.add_argument("--find-minutes", type=float, default=75)
a.add_argument("start", nargs=argparse.REMAINDER)
args = a.parse_args()
start_cmd = args.start[1:] if args.start[:1] == ["--"] else args.start
os.makedirs(args.shots, exist_ok=True)
BLUE = (0x16, 0x51, 0xB0)  # the card's Restart button (crates/app updates.rs card_button)
t0 = time.time()


def say(m):
    print(f"[{time.time() - t0:6.0f} s] {m}", flush=True)


def fail(why):
    say(f"FAILED: {why}")
    say("app log:\n" + read_log())
    shot("failed")
    summary = os.environ.get("GITHUB_STEP_SUMMARY")
    if summary:
        with open(summary, "a", encoding="utf-8") as f:
            f.write(f"### :x: The installed previous release didn't update itself to v{args.want}{' (' + args.label + ')' if args.label else ''}\n\n{why}\n\n")
    sys.exit(1)


def read_log():
    try:
        return open(args.log, encoding="utf-8", errors="replace").read()
    except OSError:
        return ""


def wait_for(lines, minutes, stop=()):
    """Every string of `lines`, in order, in the log within `minutes`; a line with a `stop` string ends the wait."""
    end, last_note = time.time() + minutes * 60, ""
    while True:
        text, want = read_log(), list(lines)
        for line in text.splitlines():
            if want and want[0] in line:
                want.pop(0)
            if any(s in line for s in stop):
                fail(f"the app said: {line.strip()}")
        if not want:
            return text
        notes = [l for l in text.splitlines() if "rate limited" in l or "check:" in l]
        if notes and notes[-1] != last_note:
            last_note = notes[-1]
            say(last_note.strip())
        if time.time() > end:
            fail(f"no log line with {want[0]!r} after {minutes:.0f} min" + (f" (last: {last_note.strip()})" if last_note else ""))
        time.sleep(2)


def shot(name):
    try:
        img = grab()
        img.save(os.path.join(args.shots, f"{name}.png"))
        return img
    except Exception as e:  # a screenshot is evidence, not the check
        say(f"screenshot {name}: {e}")


if sys.platform == "win32":
    ctypes.windll.user32.SetProcessDPIAware()


def grab():
    return ImageGrab.grab(all_screens=True) if sys.platform == "win32" else ImageGrab.grab()


def screen_scale(img):
    """Screenshot pixels per click unit (macOS clicks in points; a Retina screenshot has two pixels per point)."""
    if sys.platform != "darwin":
        return 1.0
    cg = ctypes.cdll.LoadLibrary("/System/Library/Frameworks/CoreGraphics.framework/CoreGraphics")

    class Rect(ctypes.Structure):
        _fields_ = [("x", ctypes.c_double), ("y", ctypes.c_double), ("w", ctypes.c_double), ("h", ctypes.c_double)]

    cg.CGMainDisplayID.restype = ctypes.c_uint32
    cg.CGDisplayBounds.restype = Rect
    cg.CGDisplayBounds.argtypes = [ctypes.c_uint32]
    return img.width / cg.CGDisplayBounds(cg.CGMainDisplayID()).w


def find_button(img):
    """The centre of the blue button lowest on the screen (at least 30 px wide and 12 rows tall; its white label
    breaks the rows in its middle), or None."""
    px, (w, h) = img.convert("RGB").load(), img.size
    near = lambda c: all(abs(c[i] - BLUE[i]) <= 8 for i in range(3))
    found, group = None, []
    for y in list(range(h // 3, h)) + [None]:
        row = None
        if y is not None:
            run = best = end = count = 0
            for x in range(w):
                if near(px[x, y]):
                    run, count = run + 1, count + 1
                    if run > best:
                        best, end = run, x
                else:
                    run = 0
            if best >= 30 or (best >= 6 and count >= 25):
                row = (y, end - best + 1, end, best >= 30)
        if row:
            group.append(row)
            continue
        solid = [g for g in group if g[3]]
        if len(group) >= 12 and solid:
            found = ((min(g[1] for g in solid) + max(g[2] for g in solid)) // 2, (group[0][0] + group[-1][0]) // 2)
        group = []
    return found


def click(x, y):
    if sys.platform == "win32":
        u = ctypes.windll.user32
        u.SetCursorPos(x, y)
        time.sleep(0.3)
        u.mouse_event(0x0002, 0, 0, 0, 0)
        time.sleep(0.08)
        u.mouse_event(0x0004, 0, 0, 0, 0)
    elif sys.platform == "darwin":
        cg = ctypes.cdll.LoadLibrary("/System/Library/Frameworks/CoreGraphics.framework/CoreGraphics")

        class Point(ctypes.Structure):
            _fields_ = [("x", ctypes.c_double), ("y", ctypes.c_double)]

        cg.CGEventCreateMouseEvent.restype = ctypes.c_void_p
        cg.CGEventCreateMouseEvent.argtypes = [ctypes.c_void_p, ctypes.c_uint32, Point, ctypes.c_uint32]
        cg.CGEventPost.argtypes = [ctypes.c_uint32, ctypes.c_void_p]
        for kind in (5, 1, 2):  # moved, left down, left up
            cg.CGEventPost(0, cg.CGEventCreateMouseEvent(None, kind, Point(x, y), 0))
            time.sleep(0.1)
    else:
        subprocess.run(["xdotool", "mousemove", str(x), str(y), "sleep", "0.3", "click", "1"], check=True)


# 1. The installed release and a document, then the app started as a user starts it.
v = subprocess.run([args.exe, "--version"], capture_output=True, text=True, timeout=120).stdout.strip()
say(f"installed: {v}; updating to v{args.want}")
if args.want in v.split():
    fail(f"v{args.want} is installed already: nothing to update")
if not os.path.exists(args.doc):
    r = subprocess.run([args.exe, "call", "document_create", json.dumps({"path": args.doc})], capture_output=True, text=True, timeout=300)
    if not os.path.exists(args.doc):
        fail(f"document_create: exit {r.returncode}: {r.stderr.strip()}")
if os.path.exists(args.log):
    os.remove(args.log)
env = dict(os.environ, NOSHAPE_UPDATE_LOG=args.log, NOSHAPE_NO_SETUP_PROMPT="1")
env.pop("NOSHAPE_NO_UPDATES", None)
say(f"starting: {start_cmd}")
subprocess.Popen(start_cmd, env=env, stdin=subprocess.DEVNULL, start_new_session=sys.platform != "win32")

# 2. Found on GitHub, downloaded and verified (the card says "Updated - restart to finish").
wait_for(["started "], 5)
wait_for([f"found {args.want}", f"verified {args.want}"], args.find_minutes, stop=[f"refused {args.want}", "manual:"])
say("verified; the card shows Restart")
time.sleep(3)

# 3. Restart pressed: the button found on the screen, clicked until the app says it restarts.
for attempt in range(6):
    img = shot(f"ready-{attempt}")
    at = find_button(img) if img else None
    if not at:
        say("no Restart button on the screen yet")
        time.sleep(5)
        continue
    s = screen_scale(img)
    x, y = int(at[0] / s), int(at[1] / s)
    say(f"Restart at {x},{y}")
    click(x, y)
    deadline = time.time() + 15
    while time.time() < deadline and "restarting into" not in read_log():
        time.sleep(1)
    if "restarting into" in read_log():
        break
else:
    fail("Restart never took: no button found or no restart after clicking it (screenshots in the artifact)")

# 4. The new version runs with the document and is the one installed.
text = wait_for([f"restarting into {args.want} with 1 document(s)", "helper: swapped", f"started {args.want}", "reopened 1 document(s)"], 10, stop=["update failed", "helper: failed"])
time.sleep(5)
shot("after")
v = subprocess.run([args.exe, "--version"], capture_output=True, text=True, timeout=120).stdout.strip()
if args.want not in v.split():
    fail(f"the new version runs, but {args.exe} --version says {v!r}")
say("app log:\n" + text)
say(f"OK: updated to {v} through the update card's Restart, document reopened")
summary = os.environ.get("GITHUB_STEP_SUMMARY")
if summary:
    with open(summary, "a", encoding="utf-8") as f:
        f.write(f"### :white_check_mark: The installed previous release updated itself to v{args.want}{' (' + args.label + ')' if args.label else ''}\n\nFound, downloaded and verified from GitHub, Restart pressed, document reopened ({time.time() - t0:.0f} s).\n\n")
