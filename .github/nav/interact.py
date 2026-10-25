"""Interaction replay (docs/parity/interaction.md): selection and hover in the
Part Studio of block.noshape (a 40 x 20 x 10 mm block on the Top plane, corner
at the origin, the default planes shown), driven with the same uinput devices
as the navigation replay. The app traces its selection and hovered entity
(NOSHAPE_NAV_TRACE "highlights" lines); interact_check.py asserts them per
step.
"""

import argparse
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import uinput as ui  # noqa: E402
from replay import Replay, read_trace  # noqa: E402

MM = 0.001


class Interact(Replay):
    def camera(self):
        cams = [r for r in read_trace(self.trace) if "camera" in r]
        return cams[-1]

    def project(self, p):
        """Window pixels of a world point (orthographic camera)."""
        c = self.camera()
        cam = c["camera"]
        if cam["perspective"]:
            sys.exit("the interaction replay expects an orthographic view")
        d = [p[i] - cam["target"][i] for i in range(3)]
        dot = lambda a, b: sum(x * y for x, y in zip(a, b))  # noqa: E731
        w = cam["world_per_pixel"]
        return (c["origin"][0] + c["viewport"][0] / 2 + dot(d, cam["right"]) / w,
                c["origin"][1] + c["viewport"][1] / 2 - dot(d, cam["up"]) / w)

    def settle_highlights(self, quiet=0.6, limit=10.0):
        """Until the traced highlights stay unchanged for `quiet` s."""
        end = time.time() + limit
        count = sum(1 for r in read_trace(self.trace) if "highlights" in r)
        still = time.time()
        while time.time() - still < quiet and time.time() < end:
            time.sleep(0.1)
            n = sum(1 for r in read_trace(self.trace) if "highlights" in r)
            if n != count:
                count, still = n, time.time()

    def step(self, name, fn):
        self.mark(name, "start")
        fn()
        self.settle_highlights()
        self.mark(name, "end")

    def click_at(self, p, mods=()):
        def run():
            self.cursor(*p)
            for m in mods:
                self.keys.key(m, True)
            time.sleep(0.05)
            self.mouse.button(ui.BTN_LEFT, True)
            time.sleep(0.05)
            self.mouse.button(ui.BTN_LEFT, False)
            time.sleep(0.05)
            for m in mods:
                self.keys.key(m, False)
            # Past the double-click time, so the next click is a new one.
            time.sleep(0.6)
        return run

    def box(self, a, b, steps=20):
        """A left drag from window point a to b."""
        def run():
            self.cursor(*a)
            self.mouse.button(ui.BTN_LEFT, True)
            time.sleep(0.05)
            dx, dy = (b[0] - a[0]) / steps, (b[1] - a[1]) / steps
            done = [0, 0]
            for i in range(1, steps + 1):
                tx, ty = round(dx * i), round(dy * i)
                self.mouse.move(tx - done[0], ty - done[1])
                done = [tx, ty]
                time.sleep(0.016)
            time.sleep(0.2)
            self.mouse.button(ui.BTN_LEFT, False)
            time.sleep(0.5)
        return run

    def hover(self, p):
        def run():
            self.cursor(*p)
            # A real move over the target (the cursor jump alone may not pick).
            self.mouse.move(1, 0)
            time.sleep(0.3)
        return run

    def run(self):
        self.wait_ready()
        top = self.project((20 * MM, 10 * MM, 10 * MM))
        front = self.project((20 * MM, 0, 5 * MM))
        # Empty graphics: the viewport's lower-left corner.
        c = self.camera()
        empty = (c["origin"][0] + 30, c["origin"][1] + c["viewport"][1] - 30)
        # A point of the Top plane just in front of the block (the default
        # planes are centred on the origin, their default size whatever the
        # model): nearer the camera than the Front plane (y < 0), the Right
        # plane (x > 0) and the block's front face.
        plane = self.project((20 * MM, -4 * MM, 0))
        self.step("click-top", self.click_at(top))
        self.step("click-front-adds", self.click_at(front))
        self.step("click-empty-clears", self.click_at(empty))
        self.step("hover-top", self.hover(top))
        # Out of the viewport over the feature list at its left.
        self.step("hover-leave", self.hover((c["origin"][0] / 2, c["origin"][1] + 200)))
        self.step("select-for-space", self.click_at(top))
        self.step("space-clears", self.key_tap(ui.KEY_SPACE))
        # A small crossing box (right to left) over the Top plane only.
        self.step("crossing-box-plane", self.box((plane[0] + 12, plane[1] + 6), (plane[0] - 12, plane[1] - 6)))
        self.step("clear", self.click_at(empty))
        for d in (self.mouse, self.keys, self.pad, self.screen, self.pen):
            d.close()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--trace", required=True)
    ap.add_argument("--mode", default="wayland")
    ap.add_argument("--out", default=".")
    a = ap.parse_args()
    Interact(a.trace).run()


if __name__ == "__main__":
    main()
