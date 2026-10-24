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
                pinches.append(float(args[3]))
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


def load(trace):
    with open(trace) as f:
        return [json.loads(line) for line in f if line.strip()]


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

    def report(self):
        for step, what, got, want, ok in self.rows:
            status = "info" if ok is None else ("PASS" if ok else "FAIL")
            fmt = lambda v: "-" if v is None else (f"{v:.6g}" if isinstance(v, float) else str(v))  # noqa: E731
            print(f"{status:4}  {step:22} {what:44} got {fmt(got):>12}  want {fmt(want):>12}")
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
    a = ap.parse_args()
    rec = load(a.trace)
    by = steps(rec)
    ch = Checker()
    frames, pinch_scales = ([], [])
    windows = a.mode == "windows"
    if a.wayland_log and a.mode == "wayland":
        frames, pinch_scales = wayland_scroll_frames(a.wayland_log)
    elif a.mode in ("mac", "windows"):
        # The drivers log each OS event they post; Chrome's ticks for it.
        frames = [posted_ticks(r["posted"]) for r in rec if r.get("posted", {}).get("kind") == "scroll"]
    # The app's scroll inputs in order, to line them up with the protocol.
    scrolls = [r["data"] for r in rec if r.get("input") == "scroll"]
    if frames:
        ch.check("protocol", "scroll frames = scroll inputs", float(len(scrolls)), float(len(frames)), 0)
        for i, (s, fr) in enumerate(zip(scrolls, frames)):
            got = s.get("ticks")
            ch.check("protocol", f"scroll #{i} wheelDelta y (Chrome)", None if got is None else float(trunc120(got[1])), float(trunc120(fr[1])), 0)
            ch.check("protocol", f"scroll #{i} wheelDelta x (Chrome)", None if got is None else float(trunc120(got[0])), float(trunc120(fr[0])), 0)

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
        ch.note("protocol", "pinch updates", len(pinch_scales))

    # Drags: rotation angle about the camera's up axis, pan of the target.
    def first_last(name):
        cams = [r for r in by.get(name, []) if "camera" in r]
        return (by[name][0]["camera0"], cams[-1]) if cams else (None, None)

    c0, c1 = first_last("right-drag")
    if c0 and c1:
        w, h = c1["viewport"]
        want = 3 * 360 * 200 / math.hypot(w, h)
        ch.check("right-drag", "turn (deg) for 200 px", angle(c0["camera"]["forward"], c1["camera"]["forward"]), want, 0.5)
    c0, c1 = first_last("right-drag-fling")
    if c0 and c1:
        w, h = c1["viewport"]
        ch.note("right-drag-fling", "turn (deg), 200 px + inertia", angle(c0["camera"]["forward"], c1["camera"]["forward"]))
        ch.note("right-drag-fling", "  without inertia it would be", 3 * 360 * 200 / math.hypot(w, h))
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
    ch.report()
    sys.exit(1 if ch.failed else 0)


if __name__ == "__main__":
    main()
