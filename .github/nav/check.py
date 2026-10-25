"""Compare a navigation replay (replay.py) with Onshape.

Independently of noshape's Rust code, this recomputes from the raw Wayland
protocol events the app received (WAYLAND_DEBUG) what Chrome would hand
Onshape's client (`wheelDelta`, Chromium's rules: navigation.md "Chrome's
input mapping") and what Onshape's view controller then does with it
(`zoomOnMouseWheelScroll`: 2^(-i/6) per event, i = 0.5/120 x wheelDelta; drag
rotate 3 turns per viewport diagonal; touch 2 turns; pinch = finger ratio),
and checks every camera change in the trace against it.

Exit status 1 when a check fails.
"""

import argparse
import json
import math
import re
import struct
import sys


def f32(x):
    return struct.unpack("f", struct.pack("f", x))[0]


def trunc120(ticks):
    """Blink's `wheelDelta`: int(f32(ticks) * 120) computed in f32."""
    return int(f32(f32(ticks) * 120.0))


def onshape_amount(wheel_delta_y, wheel_delta_x=0, windows=False):
    if wheel_delta_y == 0 or (wheel_delta_x and abs(wheel_delta_x) > abs(wheel_delta_y)):
        return None
    return 0.5 / 120 * (3 if windows else 1) * wheel_delta_y


EVENT = re.compile(r"\[\s*([\d.]+)\]\s+(?:\{[^}]*\}\s+)?(?!->)([a-z_0-9]+)[@#](\d+)\.([a-z_0-9]+)\((.*)\)\s*$")


def wayland_scroll_frames(log):
    """Chrome's per-frame scroll from the client's wl_pointer events: a list
    of (wheel_ticks_x, wheel_ticks_y) as wayland_event_source.cc builds them."""
    frames, pinches = [], []
    data, source = None, "wheel"  # data: [dx, dy, is_high_resolution] for this frame

    def axis_event(offset, high_resolution):
        # WaylandEventSource::OnPointerAxisEvent
        nonlocal data
        if data is None:
            data = [0.0, 0.0, False]
        if high_resolution == data[2]:
            data[0] += offset[0]
            data[1] += offset[1]
        elif high_resolution:
            data = [offset[0], offset[1], True]

    with open(log, errors="replace") as f:
        for line in f:
            m = EVENT.search(line)
            if not m:
                continue
            iface, ev, args = m.group(2), m.group(4), [a.strip() for a in m.group(5).split(",")]
            if iface == "wl_pointer":
                if ev == "axis_source":
                    source = {"0": "wheel", "1": "finger", "2": "continuous", "3": "wheel_tilt"}.get(args[0], "wheel")
                elif ev == "axis":
                    # WaylandPointer::OnAxis: -value / 10 x 120 (unscaled scrolling off)
                    axis, value = int(args[1]), float(args[2])
                    d = -value * 12.0
                    axis_event((d, 0.0) if axis == 1 else (0.0, d), False)
                elif ev == "axis_value120":
                    axis, v120 = int(args[0]), -float(args[1])
                    axis_event((v120, 0.0) if axis == 1 else (0.0, v120), True)
                elif ev == "frame":
                    if data is not None:
                        if source in ("wheel", "wheel_tilt"):
                            frames.append((round(data[0]) / 120.0, round(data[1]) / 120.0))
                        else:
                            frames.append((data[0] / 120.0, data[1] / 120.0))
                    data = None
            elif iface == "zwp_pointer_gesture_pinch_v1" and ev == "update":
                # One per compositor event: a client holding two pinch
                # objects sees each update twice (same time); Chrome has one.
                if not pinches or pinches[-1][0] != args[0]:
                    pinches.append((args[0], float(args[3])))
    return frames, pinches


def posted_ticks(p):
    """Chromium's wheel ticks for an event a driver posted: macOS continuous
    point deltas / 40 or the notched DeltaAxis (web_input_event_builders_mac),
    Windows WHEEL_DELTA units / 120 (web_input_event_builders_win)."""
    if "continuous" in p:
        if p["continuous"]:
            return (f32(p["axis2"] / 40.0), f32(p["axis1"] / 40.0))
        return (float(p["axis2"]), float(p["axis1"]))
    if p.get("horizontal"):
        return (-p["wheel"] / 120.0, 0.0)
    return (0.0, p["wheel"] / 120.0)


BAD_LINES = []


def load(trace):
    out = []
    with open(trace) as f:
        for line in f:
            if line.strip():
                try:
                    out.append(json.loads(line))
                except ValueError:
                    BAD_LINES.append(line)
    return out


def steps(records):
    """{step name: [records]} between the step's start and end marks, with
    the camera current at the start prepended as {"camera0": ...}."""
    out, cur, cam = {}, None, None
    for r in records:
        if "camera" in r:
            cam = r
        if "step" in r:
            if r["at"] == "start":
                cur = r["step"]
                out[cur] = [{"camera0": cam}]
            else:
                cur = None
            continue
        if cur:
            out[cur].append(r)
    return out


def world_under(cam, at):
    c, (w, h) = cam["camera"], cam["viewport"]
    wpp = c["world_per_pixel"]
    return [c["target"][i] + c["right"][i] * (at[0] - w / 2) * wpp - c["up"][i] * (at[1] - h / 2) * wpp for i in range(3)]


def angle(a, b):
    d = sum(x * y for x, y in zip(a, b))
    return math.degrees(math.acos(max(-1.0, min(1.0, d))))


class Checker:
    def __init__(self):
        self.failed = 0
        self.rows = []

    def check(self, step, what, got, want, tol):
        ok = got is not None and want is not None and abs(got - want) <= tol
        if not ok:
            self.failed += 1
        self.rows.append((step, what, got, want, ok))

    def note(self, step, what, value):
        self.rows.append((step, what, value, None, None))

    def report(self, verbose=False):
        fmt = lambda v: "-" if v is None else (f"{v:.6g}" if isinstance(v, float) else str(v))  # noqa: E731
        per_step = {}
        for step, what, got, want, ok in self.rows:
            status = "info" if ok is None else ("PASS" if ok else "FAIL")
            if ok is not None:
                n = per_step.setdefault(step, [0, 0])
                n[0 if ok else 1] += 1
            if verbose or ok is not True:
                print(f"{status:4}  {step:22} {what:44} got {fmt(got):>12}  want {fmt(want):>12}")
        for step, (good, bad) in per_step.items():
            print(f"{'PASS' if not bad else 'FAIL':4}  {step:22} {good} checks passed, {bad} failed")
        print(f"{len([r for r in self.rows if r[4] is not None])} checks, {self.failed} failed")


def check_zoom_events(ch, name, recs, kind, expect_amount):
    """Each scroll / pinch input followed by a camera: the zoom ratio and the
    fixed point under the pointer."""
    cam = recs[0]["camera0"]
    pending = None
    n = 0
    for r in recs[1:]:
        if r.get("input") == kind:
            if pending is not None:
                # The previous input changed nothing.
                want = expect_amount(pending, n - 1)
                ch.check(name, f"#{n - 1} {kind} zoom (no camera change)", 0.0, 0.0 if want is None else want, 1e-9)
            pending = r["data"]
            n += 1
        elif "camera" in r and pending is not None:
            i = expect_amount(pending, n - 1)
            ratio = r["camera"]["world_per_pixel"] / cam["camera"]["world_per_pixel"]
            got = -6.0 * math.log2(ratio)
            ch.check(name, f"#{n - 1} {kind} amount i (from wpp ratio)", got, i, 1e-6)
            if cam is not None:
                a, b = world_under(cam, pending["at"]), world_under(r, pending["at"])
                ch.check(name, f"#{n - 1} point under pointer moved (wpp)", math.dist(a, b) / r["camera"]["world_per_pixel"], 0.0, 0.5)
            cam, pending = r, None
        elif "camera" in r:
            cam = r
    if pending is not None:
        want = expect_amount(pending, n - 1)
        ch.check(name, f"#{n - 1} {kind}: no zoom", 0.0, 0.0 if want is None else want, 1e-9)
    return n


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--trace", required=True)
    ap.add_argument("--wayland-log")
    ap.add_argument("--mode", default="wayland")
    ap.add_argument("--verbose", action="store_true")
    a = ap.parse_args()
    rec = load(a.trace)
    by = steps(rec)
    ch = Checker()
    ch.check("trace", "malformed lines (interleaved writes)", float(len(BAD_LINES)), 0.0, 0)
    frames, pinch_scales = ([], [])
    windows = a.mode == "windows"
    if a.wayland_log and a.mode == "wayland":
        frames, pinch_scales = wayland_scroll_frames(a.wayland_log)

    # The app's scroll inputs in order, to line them up with the protocol.
    scrolls = [r["data"] for r in rec if r.get("input") == "scroll"]
    if frames:
        ch.check("protocol", "scroll frames = scroll inputs", float(len(scrolls)), float(len(frames)), 0)
        for i, (s, fr) in enumerate(zip(scrolls, frames)):
            got = s.get("ticks")
            ch.check("protocol", f"scroll #{i} wheelDelta y (Chrome)", None if got is None else float(trunc120(got[1])), float(trunc120(fr[1])), 0)
            ch.check("protocol", f"scroll #{i} wheelDelta x (Chrome)", None if got is None else float(trunc120(got[0])), float(trunc120(fr[0])), 0)

    baselines = set()
    if a.mode == "x11":
        seen = set()
        for r in rec:
            if r.get("input") == "mouse_move":
                seen.clear()
            p_ = r.get("posted")
            if p_ and p_.get("kind") == "scroll":
                axis = "x" if p_.get("horizontal") else "y"
                if axis not in seen:
                    baselines.add(id(r))
                seen.add(axis)
    # The drivers log each wheel event they post; per step, Chrome's ticks for
    # each must equal the ticks the app's platform layer reported.
    for name, recs in by.items():
        posted = [posted_ticks(r["posted"]) for r in recs if r.get("posted", {}).get("kind") == "scroll"]
        if not posted:
            continue
        got = [r["data"] for r in recs if r.get("input") == "scroll"]
        if a.mode == "x11":
            # XI2 smooth scrolling: an axis's first valuator event after the
            # device switched (the replay's cursor warp) or the pointer entered
            # only sets the baseline, in Chromium (`ExtractAndUpdateScrollOffset`)
            # as in gpui. Drop those posted events from the expectation.
            kept = [posted_ticks(r["posted"]) for r in recs if r.get("posted", {}).get("kind") == "scroll" and id(r) not in baselines]
            ch.note(name, "baseline events (no scroll on X11)", len(posted) - len(kept))
            posted = kept
        shifted = [r["posted"].get("shift", False) for r in recs if r.get("posted", {}).get("kind") == "scroll" and id(r) not in baselines]
        # Compositors may merge wheel events posted in quick succession into
        # one frame (libinput/sway sum the value120s, and drop a frame that
        # sums to 0; the app and Chrome see the merged frames). So compare
        # running totals: after each app event its total wheelDelta must equal
        # the posted total at some later posted event, and the final totals
        # must agree.
        if a.mode == "mac" and any(shifted):
            # macOS may move a Shift+wheel notch to the horizontal axis on
            # delivery (Chrome reads the same event); notches aren't merged.
            ch.check(name, "scroll inputs = posted wheel events", float(len(got)), float(len(posted)), 0)
            for i, (s_, p_) in enumerate(zip(got, posted)):
                t = s_.get("ticks") or (0.0, 0.0)
                ch.check(name, f"#{i} wheelDelta on either axis", float(trunc120(t[0]) + trunc120(t[1])), float(trunc120(p_[0]) + trunc120(p_[1])), 0)
            continue
        totals, tx, ty = [], 0, 0
        for p_ in posted:
            tx, ty = tx + trunc120(p_[0]), ty + trunc120(p_[1])
            totals.append((tx, ty))
        ax = ay = 0
        k = 0
        for i, s_ in enumerate(got):
            t = s_.get("ticks")
            if t is None:
                ch.check(name, f"#{i} wheel ticks reported", 0.0, 1.0, 0)
                continue
            ax, ay = ax + trunc120(t[0]), ay + trunc120(t[1])
            while k < len(totals) and (ax, ay) != totals[k]:
                k += 1
            ch.check(name, f"#{i} running wheelDelta ({ax}, {ay}) = a posted running total", float(k < len(totals)), 1.0, 0)
            k += 1
        ch.check(name, "total wheelDelta y = posted total", float(ay), float(ty), 0)
        ch.check(name, "total wheelDelta x = posted total", float(ax), float(tx), 0)

    def wheel_amount(d, _n):
        t = d.get("ticks")
        if t is None:
            return None
        return onshape_amount(trunc120(t[1]), trunc120(t[0]), windows)

    for name in ("wheel", "shift-wheel", "ctrl-wheel", "pad-scroll-up", "pad-scroll-down", "pad-scroll-sideways"):
        if name in by:
            n = check_zoom_events(ch, name, by[name], "scroll", wheel_amount)
            ch.note(name, "scroll events", n)

    def pinch_amount(d, _n):
        if d["phase"] != "Moved" or a.mode == "x11":
            return None
        return (1.5 if windows else 0.5) * (1 if f32(1.0 + d["delta"]) > 1.0 else -1)

    for name in ("pad-pinch-out", "pad-pinch-in"):
        if name in by:
            n = check_zoom_events(ch, name, by[name], "pinch", pinch_amount)
            ch.note(name, "pinch events", n)
    if pinch_scales:
        moved = [r for r in rec if r.get("input") == "pinch" and r["data"]["phase"] == "Moved"]
        ch.check("protocol", "pinch updates = app pinch updates", float(len(moved)), float(len(pinch_scales)), 0)

    # Drags: rotation angle about the camera's up axis, pan of the target.
    def first_last(name):
        cams = [r for r in by.get(name, []) if "camera" in r]
        return (by[name][0]["camera0"], cams[-1]) if cams else (None, None)

    # Steps that must move the camera: a missing camera change must fail,
    # not skip their checks.
    for name in ("right-drag", "right-drag-fling", "middle-drag", "alt-right-drag", "ctrl-mid-drag", "second-button-mid-drag",
                 "release-outside", "touch-rotate", "touch-pan", "touch-pinch", "touch-roll", "touch-view-cube", "pen-rotate"):
        if name in by and first_last(name)[1] is None:
            ch.check(name, "the camera moved", 0.0, 1.0, 0)

    c0, c1 = first_last("right-drag")
    if c0 and c1:
        w, h = c1["viewport"]
        want = 3 * 360 * 200 / math.hypot(w, h)
        ch.check("right-drag", "turn (deg) for 200 px", angle(c0["camera"]["forward"], c1["camera"]["forward"]), want, 0.5)
    # The rotate centre (the geometry under the pointer at the press, else the
    # visible-depth average) stays put on screen while the view turns.
    pivots = [r["data"]["pivot"] for r in by.get("right-drag", []) if r.get("input") == "pivot" and r["data"].get("pivot")]
    if c0 and c1 and pivots:
        def screen(p, cam):
            c, (w, h) = cam["camera"], cam["viewport"]
            d = [p[i] - c["target"][i] for i in range(3)]
            wpp = c["world_per_pixel"]
            return (sum(d[i] * c["right"][i] for i in range(3)) / wpp + w / 2, -sum(d[i] * c["up"][i] for i in range(3)) / wpp + h / 2)
        a0, a1 = screen(pivots[-1], c0), screen(pivots[-1], c1)
        ch.check("right-drag", "rotate centre moved on screen (px)", math.dist(a0, a1), 0.0, 0.01)
    c0, c1 = first_last("right-drag-fling")
    if c0 and c1:
        w, h = c1["viewport"]
        ch.note("right-drag-fling", "turn (deg), 200 px + inertia", angle(c0["camera"]["forward"], c1["camera"]["forward"]))
        ch.note("right-drag-fling", "  without inertia it would be", 3 * 360 * 200 / math.hypot(w, h))
    if "alt-right-drag" in by:
        # Constrained rotate: the view is first levelled (snapped, or animated
        # while drag moves are ignored), which tags it; only moves after that
        # turn it about world Z, 3 turns per diagonal (Onshape onDrag
        # isAxisRotating, axisRotateCanvasTag). How many of the 10 px moves
        # fall after the levelling depends on timing, so the check is that the
        # turn is a whole number of moves, about Z, with the view level.
        cams = [r for r in by["alt-right-drag"][1:] if "camera" in r]
        level = [r for r in cams if abs(r["camera"]["right"][2]) < 1e-9]
        if level:
            c0, c1 = level[0], cams[-1]
            w, h = c1["viewport"]
            az = lambda f: math.degrees(math.atan2(f[1], f[0]))  # noqa: E731
            turn = (az(c1["camera"]["forward"]) - az(c0["camera"]["forward"])) % 360
            turn = min(turn, 360 - turn)
            per_move = 3 * 360 * 10 / math.hypot(w, h)
            k = round(turn / per_move)
            ch.check("alt-right-drag", "turn = whole 10 px moves at 3 turns/diagonal (deg)", turn, k * per_move, 0.01)
            ch.note("alt-right-drag", "moves that turned the levelled view", k)
            el = lambda c: math.degrees(math.asin(c["camera"]["forward"][2]))  # noqa: E731
            ch.check("alt-right-drag", "elevation change after levelling (deg)", abs(el(c1) - el(c0)), 0.0, 0.01)
            ch.check("alt-right-drag", "right vector stays horizontal", abs(c1["camera"]["right"][2]), 0.0, 1e-9)
        else:
            ch.check("alt-right-drag", "view levelled", 0.0, 1.0, 0)
    for name in ("ctrl-mid-drag", "second-button-mid-drag"):
        c0, c1 = first_last(name)
        if c0 and c1:
            # -100 px rotate, +100 px pan (Ctrl / middle held), -100 px rotate:
            # the view turns by 200 px worth (100 px if the switch were ignored).
            w, h = c1["viewport"]
            ch.check(name, "turn (deg) = 200 px of rotate moves", angle(c0["camera"]["forward"], c1["camera"]["forward"]), 3 * 360 * 200 / math.hypot(w, h), 0.5)
    if "wheel-during-box" in by:
        n = len([r for r in by["wheel-during-box"] if r.get("input") == "scroll"])
        cams = [r for r in by["wheel-during-box"] if "camera" in r]
        ch.check("wheel-during-box", f"camera unchanged by {n} wheel events", float(len(cams)), 0.0, 0)
    if "release-outside" in by:
        recs = by["release-outside"]
        i = next((k for k, r in enumerate(recs) if r.get("mark") == "released"), None)
        if i is not None:
            before = [r for r in recs[:i] if "camera" in r]
            after = [r for r in recs[i:] if "camera" in r]
            ch.check("release-outside", "the drag rotated before the release", float(bool(before)), 1.0, 0)
            down = next((r["data"]["at"] for r in recs if r.get("input") == "mouse_down"), None)
            up = next((r["data"]["at"] for r in recs if r.get("input") == "mouse_up"), None)
            c0 = recs[0]["camera0"]
            if down and up and before and c0:
                # Onshape follows the drag over the whole page: the moves over
                # the panel rotate too.
                w, h = before[-1]["viewport"]
                want = (3 * 360 * abs(up[0] - down[0]) / math.hypot(w, h)) % 360
                want = min(want, 360 - want)
                ch.check("release-outside", f"turn (deg) for the whole {abs(up[0] - down[0]):.0f} px drag", angle(c0["camera"]["forward"], before[-1]["camera"]["forward"]), want, 0.5)
            ch.check("release-outside", "camera changes after the release", float(len(after)), 0.0, 0)
    c0, c1 = first_last("middle-drag")
    if c0 and c1:
        moved = math.dist(c0["camera"]["target"], c1["camera"]["target"]) / c1["camera"]["world_per_pixel"]
        ch.check("middle-drag", "pan (px) for a (200, 100) drag", moved, math.hypot(200, 100), 1.0)
    c0, c1 = first_last("touch-rotate")
    if c0 and c1:
        w, h = c1["viewport"]
        ch.note("touch-rotate", "turn (deg) for 200 px + inertia", angle(c0["camera"]["forward"], c1["camera"]["forward"]))
        ch.note("touch-rotate", "  2 turns per diagonal, no inertia", 2 * 360 * 200 / math.hypot(w, h))
    c0, c1 = first_last("touch-pan")
    if c0 and c1:
        moved = math.dist(c0["camera"]["target"], c1["camera"]["target"]) / c1["camera"]["world_per_pixel"]
        ch.check("touch-pan", "pan (px) for a 150 px two-finger move", moved, 150.0, 2.0)
    c0, c1 = first_last("touch-pinch")
    if c0 and c1:
        ch.check("touch-pinch", "zoom ratio for fingers 100 -> 200 px", c0["camera"]["world_per_pixel"] / c1["camera"]["world_per_pixel"], 2.0, 0.02)
    c0, c1 = first_last("touch-roll")
    if c0 and c1:
        ch.check("touch-roll", "roll (deg) for a 30 deg finger turn", angle(c0["camera"]["up"], c1["camera"]["up"]), 30.0, 1.0)
    # Keys (Onshape's rotateInDirection / animatePan / animateZoom): exact
    # angles after the 250 ms animations, the pan at the scene centre, ×1.75.
    for name, deg in (("key-left", 15.0), ("key-up", 15.0), ("key-shift-left", 90.0), ("key-ctrl-left", 5.0)):
        c0, c1 = first_last(name)
        if name in by and c1 is None:
            ch.check(name, "the camera moved", 0.0, 1.0, 0)
        elif c0 and c1:
            ch.check(name, f"turn (deg) = {deg:g}", angle(c0["camera"]["forward"], c1["camera"]["forward"]), deg, 1e-6)
    c0, c1 = first_last("key-ctrl-shift-right")
    if c0 and c1:
        # The model moves 150 px right: the target 150 px worth to the left.
        d = [c1["camera"]["target"][i] - c0["camera"]["target"][i] for i in range(3)]
        moved = sum(d[i] * c0["camera"]["right"][i] for i in range(3)) / c0["camera"]["world_per_pixel"]
        ch.check("key-ctrl-shift-right", "target moves (px along right)", moved, -150.0, 1e-3)
        ch.check("key-ctrl-shift-right", "no rotation (deg)", angle(c0["camera"]["forward"], c1["camera"]["forward"]), 0.0, 1e-6)
    for name, ratio in (("key-z", 1.75), ("key-shift-z", 1 / 1.75)):
        c0, c1 = first_last(name)
        if name in by and c1 is None:
            ch.check(name, "the camera moved", 0.0, 1.0, 0)
        elif c0 and c1:
            ch.check(name, "world per pixel ratio", c1["camera"]["world_per_pixel"] / c0["camera"]["world_per_pixel"], ratio, 1e-9)
    # The pen acts as a finger (Onshape: pointerType "pen" → its touch
    # recognizers): a drag turns 2 turns per diagonal, the first 10 px move
    # only crossing the drag threshold; a tap is a one-finger tap; hovering
    # moves nothing.
    c0, c1 = first_last("pen-rotate")
    if c0 and c1:
        # From the pen positions the app received (libinput may smooth tablet
        # motion; Chrome gets the same positions): the move that started the
        # drag only crosses the threshold, every later one rotates.
        xs, start_x = [], None
        for r in by["pen-rotate"]:
            if r.get("input") == "touch":
                m = re.search(r"phase: (\w+), position: Point \{ x: ([-\d.]+)px", r["data"]["debug"])
                if m and m.group(1) in ("Started", "Moved"):
                    xs.append(float(m.group(2)))
            elif r.get("input") == "touch_action" and "DragStart" in r["data"]["debug"] and xs:
                start_x = xs[-1]
        if start_x is not None and xs:
            w, h = c1["viewport"]
            px = abs(xs[-1] - start_x)
            ch.check("pen-rotate", f"turn (deg) = {px:.1f} px of delivered moves at 2 turns/diagonal", angle(c0["camera"]["forward"], c1["camera"]["forward"]), 2 * 360 * px / math.hypot(w, h), 0.5)
        else:
            ch.check("pen-rotate", "a pen drag was recognized", 0.0, 1.0, 0)
    if "pen-tap" in by:
        actions = [r["data"]["debug"] for r in by["pen-tap"] if r.get("input") == "touch_action"]
        ch.check("pen-tap", "recognized as a one-finger tap", float(any("fingers: 1" in x for x in actions)), 1.0, 0)
    if "pen-hover" in by:
        recs = by["pen-hover"]
        ch.check("pen-hover", "camera unchanged", float(len([r for r in recs if "camera" in r])), 0.0, 0)
        ch.check("pen-hover", "hover moves reach the viewport", float(len([r for r in recs if r.get("input") == "mouse_move"]) > 0), 1.0, 0)
    # Taps and the two-finger long press (Onshape's tap recognizer): what the
    # recognizer reported, and that the camera did not move.
    for name, want in (("touch-tap", "fingers: 1"), ("touch-two-finger-tap", "fingers: 2"), ("touch-long-press", "BoxStart")):
        if name not in by:
            continue
        actions = [r["data"]["debug"] for r in by[name] if r.get("input") == "touch_action"]
        ch.check(name, f"recognized ({want})", float(any(want in x for x in actions)), 1.0, 0)
        if want.startswith("fingers"):
            ch.check(name, "one tap", float(sum("Tap" in x for x in actions)), 1.0, 0)
        cams = [r for r in by[name] if "camera" in r]
        ch.check(name, "camera unchanged", float(len(cams)), 0.0, 0)
        ch.note(name, "actions", "; ".join(actions))
    if "touch-view-cube" in by:
        # The viewport's recognizer must not see it; the cube turns the view
        # to one of its standard views (face, edge or corner).
        recs = by["touch-view-cube"]
        ch.check("touch-view-cube", "viewport gestures (must be none)", float(len([r for r in recs if r.get("input") == "touch_action"])), 0.0, 0)
        cams = [r for r in recs if "camera" in r]
        if cams:
            # A face (one axis), an edge (two equal components) or a corner
            # (our cube's corner views are trimetric-like: 0.433, 0.5, 0.75).
            f = sorted(abs(x) for x in cams[-1]["camera"]["forward"])
            views = ([0.0, 0.0, 1.0], [0.0, 2 ** -0.5, 2 ** -0.5], [3 ** -0.5] * 3, [0.4330127, 0.5, 0.75])
            off = min(max(abs(a_ - b_) for a_, b_ in zip(f, v)) for v in views)
            ch.check("touch-view-cube", "a view cube view (forward component error)", off, 0.0, 1e-6)
        else:
            ch.check("touch-view-cube", "camera changed", 0.0, 1.0, 0)
    ch.report(a.verbose)
    sys.exit(1 if ch.failed else 0)


if __name__ == "__main__":
    main()
