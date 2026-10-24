"""Drive noshape's 3D viewport with real kernel input devices (uinput) under a
headless sway and mark each step in the app's navigation trace.

Every step is bracketed by {"step": name, "at": "start"|"end"} lines appended
to the trace (the app appends its own input and camera lines in between), so
check.py can compare each step's camera changes with Onshape's numbers.
"""

import argparse
import json
import os
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import uinput as ui  # noqa: E402


def read_trace(path):
    out = []
    try:
        with open(path) as f:
            for line in f:
                try:
                    out.append(json.loads(line))
                except ValueError:
                    pass  # a line still being written
    except FileNotFoundError:
        pass
    return out


class Replay:
    def __init__(self, trace):
        self.trace = trace
        self.mouse = ui.Mouse()
        self.keys = ui.Keyboard()
        self.pad = ui.Touchpad()
        self.screen = ui.Touchscreen()
        self.origin = self.size = None

    def wait_ready(self, timeout=180):
        """Until the viewport has drawn a camera, then let the model settle."""
        end = time.time() + timeout
        while time.time() < end:
            cams = [r for r in read_trace(self.trace) if "camera" in r and min(r["viewport"]) > 100]
            if cams:
                self.origin, self.size = cams[-1]["origin"], cams[-1]["viewport"]
                time.sleep(8)
                return
            # The viewport only traces a camera once it changes: nudge it
            # with a zero-sum wheel away from any panel.
            subprocess.run(["swaymsg", "seat", "seat0", "cursor", "set", "800", "500"], capture_output=True)
            self.mouse.wheel(1)
            time.sleep(0.3)
            self.mouse.wheel(-1)
            time.sleep(2)
        sys.exit("noshape never traced a camera")

    def mark(self, name, at):
        with open(self.trace, "a") as f:
            f.write(json.dumps({"step": name, "at": at, "time": time.time()}) + "\n")

    def screen_point(self, dx=0.0, dy=0.0):
        """Window coordinates of the viewport centre + (dx, dy)."""
        return self.origin[0] + self.size[0] / 2 + dx, self.origin[1] + self.size[1] / 2 + dy

    def cursor(self, x, y):
        subprocess.run(["swaymsg", "seat", "seat0", "cursor", "set", str(int(x)), str(int(y))], check=True, capture_output=True)
        time.sleep(0.15)

    def step(self, name, fn):
        self.mark(name, "start")
        fn()
        time.sleep(0.6)
        self.mark(name, "end")

    # ------------------------------------------------------------ steps

    def post_wheel(self, hi, horizontal=False):
        """A wheel movement of `hi` 120ths, logged as posted (WHEEL_DELTA-like
        units: libinput passes them on as value120, Xwayland as valuators)."""
        with open(self.trace, "a") as f:
            f.write(json.dumps({"posted": {"kind": "scroll", "wheel": hi, "horizontal": horizontal}}) + "\n")
        self.mouse.wheel(hi_res=hi, horizontal=horizontal)
        time.sleep(0.12)

    def wheel(self):
        self.cursor(*self.screen_point(150, 80))
        for hi in (120, 120, 120, -120, -120, -120, 30, 30, 30, 30, -30, -30, -30, -30, 7, -7):
            self.post_wheel(hi)
        self.post_wheel(120, horizontal=True)

    def modifier_wheel(self, key):
        def run():
            self.cursor(*self.screen_point(-120, -60))
            self.keys.key(key, True)
            time.sleep(0.1)
            for hi in (120, -120):
                self.post_wheel(hi)
            self.keys.key(key, False)
        return run

    def pad_scroll(self, dy, dx=0):
        def run():
            self.cursor(*self.screen_point(100, -50))
            c = (2000, 1300)
            start = {0: (c[0] - 150, c[1]), 1: (c[0] + 150, c[1])}
            end = {k: (x + dx, y + dy) for k, (x, y) in start.items()}
            self.pad.gesture(start, end, steps=25)
            time.sleep(0.8)  # any kinetic scrolling
        return run

    def pad_pinch(self, spread_from, spread_to):
        def run():
            self.cursor(*self.screen_point(-80, 40))
            c = (2000, 1300)
            start = {0: (c[0] - spread_from / 2, c[1]), 1: (c[0] + spread_from / 2, c[1])}
            end = {0: (c[0] - spread_to / 2, c[1]), 1: (c[0] + spread_to / 2, c[1])}
            self.pad.gesture(start, end, steps=25)
        return run

    def touch(self, start_px, end_px, steps=20):
        """Touchscreen fingers in window pixels (10 device units per pixel)."""
        def run():
            to = lambda p: (p[0] * 10.0, p[1] * 10.0)  # noqa: E731
            self.screen.gesture({k: to(v) for k, v in start_px.items()}, {k: to(v) for k, v in end_px.items()}, steps=steps, interval=0.016)
            time.sleep(1.2)  # inertia
        return run

    def drag(self, button, dx, dy, steps=20, pause=0.1):
        def run():
            self.cursor(*self.screen_point())
            self.mouse.button(button, True)
            time.sleep(0.05)
            for _ in range(steps):
                self.mouse.move(dx // steps, dy // steps)
                time.sleep(0.016)
            time.sleep(pause)
            self.mouse.button(button, False)
            time.sleep(1.2 if pause < 0.05 else 0.2)
        return run

    def with_key(self, key, fn):
        def run():
            self.keys.key(key, True)
            time.sleep(0.1)
            fn()
            self.keys.key(key, False)
        return run

    def run(self):
        self.wait_ready()
        c = self.screen_point()
        self.step("wheel", self.wheel)
        self.step("shift-wheel", self.modifier_wheel(ui.KEY_LEFTSHIFT))
        self.step("ctrl-wheel", self.modifier_wheel(ui.KEY_LEFTCTRL))
        self.step("pad-scroll-up", self.pad_scroll(-600))
        self.step("pad-scroll-down", self.pad_scroll(600))
        self.step("pad-scroll-sideways", self.pad_scroll(0, 800))
        self.step("pad-pinch-out", self.pad_pinch(600, 1400))
        self.step("pad-pinch-in", self.pad_pinch(1400, 600))
        self.step("right-drag", self.drag(ui.BTN_RIGHT, -200, 0))
        self.step("right-drag-fling", self.drag(ui.BTN_RIGHT, -200, 0, pause=0.0))
        self.step("middle-drag", self.drag(ui.BTN_MIDDLE, 200, 100))
        self.step("alt-right-drag", self.with_key(ui.KEY_LEFTALT, self.drag(ui.BTN_RIGHT, -200, 0)))
        self.step("touch-rotate", self.touch({0: c}, {0: (c[0] - 200, c[1])}))
        self.step("touch-pan", self.touch({0: (c[0] - 50, c[1]), 1: (c[0] + 50, c[1])}, {0: (c[0] + 100, c[1]), 1: (c[0] + 200, c[1])}, steps=15))
        self.step("touch-pinch", self.touch({0: (c[0] - 50, c[1]), 1: (c[0] + 50, c[1])}, {0: (c[0] - 100, c[1]), 1: (c[0] + 100, c[1])}))
        self.step("touch-roll", self.touch({0: (c[0] - 80, c[1]), 1: (c[0] + 80, c[1])},
                                           {0: (c[0] - 80 * 0.866, c[1] - 80 * 0.5), 1: (c[0] + 80 * 0.866, c[1] + 80 * 0.5)}, steps=15))
        for d in (self.mouse, self.keys, self.pad, self.screen):
            d.close()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--trace", required=True)
    ap.add_argument("--mode", default="wayland")
    ap.add_argument("--out", default=".")
    a = ap.parse_args()
    Replay(a.trace).run()


if __name__ == "__main__":
    main()
