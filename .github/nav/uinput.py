"""Virtual Linux input devices through the kernel's uinput API (no dependencies).

Used by the navigation replay test (.github/nav/linux-replay.sh): a wheel
mouse, a multi-touch touchpad and a touchscreen that libinput (in the
compositor) treats exactly like real hardware. Every write is a real evdev
frame, so the compositor's libinput does its own gesture recognition (two-
finger scroll, pinch, touch) and the app sees what it would see on a laptop.
"""

import ctypes
import fcntl
import os
import struct
import time

EV_SYN, EV_KEY, EV_REL, EV_ABS = 0x00, 0x01, 0x02, 0x03
SYN_REPORT = 0
BTN_LEFT, BTN_RIGHT, BTN_MIDDLE = 0x110, 0x111, 0x112
BTN_TOOL_FINGER, BTN_TOUCH = 0x145, 0x14A
BTN_TOOL_DOUBLETAP, BTN_TOOL_TRIPLETAP, BTN_TOOL_QUADTAP = 0x14D, 0x14E, 0x14F
REL_X, REL_Y, REL_HWHEEL, REL_WHEEL = 0x00, 0x01, 0x06, 0x08
REL_WHEEL_HI_RES, REL_HWHEEL_HI_RES = 0x0B, 0x0C
ABS_X, ABS_Y = 0x00, 0x01
ABS_MT_SLOT, ABS_MT_POSITION_X, ABS_MT_POSITION_Y, ABS_MT_TRACKING_ID = 0x2F, 0x35, 0x36, 0x39
INPUT_PROP_POINTER, INPUT_PROP_DIRECT, INPUT_PROP_BUTTONPAD = 0x00, 0x01, 0x02
BUS_VIRTUAL = 0x06


def _ioc(direction, nr, size):
    return (direction << 30) | (size << 16) | (ord("U") << 8) | nr


UI_DEV_CREATE = _ioc(0, 1, 0)
UI_DEV_DESTROY = _ioc(0, 2, 0)
UI_DEV_SETUP = _ioc(1, 3, 92)  # struct uinput_setup
UI_ABS_SETUP = _ioc(1, 4, 28)  # struct uinput_abs_setup
UI_SET_EVBIT = _ioc(1, 100, 4)
UI_SET_KEYBIT = _ioc(1, 101, 4)
UI_SET_RELBIT = _ioc(1, 102, 4)
UI_SET_ABSBIT = _ioc(1, 103, 4)
UI_SET_PROPBIT = _ioc(1, 110, 4)


class Device:
    """One uinput device. `abs_axes` maps code -> (min, max, resolution)."""

    def __init__(self, name, keys=(), rels=(), abs_axes=None, props=(), product=0x1):
        self.fd = os.open("/dev/uinput", os.O_WRONLY | os.O_NONBLOCK)
        abs_axes = abs_axes or {}
        for ev, bits, req in ((EV_KEY, keys, UI_SET_KEYBIT), (EV_REL, rels, UI_SET_RELBIT), (EV_ABS, abs_axes, UI_SET_ABSBIT)):
            if bits:
                fcntl.ioctl(self.fd, UI_SET_EVBIT, ev)
                for b in bits:
                    fcntl.ioctl(self.fd, req, b)
        for p in props:
            fcntl.ioctl(self.fd, UI_SET_PROPBIT, p)
        for code, (lo, hi, res) in abs_axes.items():
            # struct uinput_abs_setup { u16 code; (pad); struct input_absinfo { s32 value, min, max, fuzz, flat, resolution } }
            fcntl.ioctl(self.fd, UI_ABS_SETUP, struct.pack("<HxxiiiiII", code, 0, lo, hi, 0, 0, res))
        setup = struct.pack("<HHHH80sI", BUS_VIRTUAL, 0x6E73, product, 1, name.encode()[:79], 0)
        fcntl.ioctl(self.fd, UI_DEV_SETUP, setup)
        fcntl.ioctl(self.fd, UI_DEV_CREATE)
        self.name = name
        time.sleep(0.5)  # udev + libinput pick the device up

    def emit(self, *events):
        """Write events (type, code, value) followed by a SYN_REPORT."""
        buf = b"".join(struct.pack("<qqHHi", 0, 0, t, c, v) for t, c, v in events)
        buf += struct.pack("<qqHHi", 0, 0, EV_SYN, SYN_REPORT, 0)
        os.write(self.fd, buf)

    def close(self):
        fcntl.ioctl(self.fd, UI_DEV_DESTROY)
        os.close(self.fd)


class Mouse(Device):
    """A wheel mouse with high-resolution wheels (120 per notch)."""

    def __init__(self):
        super().__init__("noshape-nav mouse", keys=(BTN_LEFT, BTN_RIGHT, BTN_MIDDLE),
                         rels=(REL_X, REL_Y, REL_WHEEL, REL_HWHEEL, REL_WHEEL_HI_RES, REL_HWHEEL_HI_RES), product=0x10)

    def move(self, dx, dy):
        self.emit((EV_REL, REL_X, dx), (EV_REL, REL_Y, dy))

    def button(self, code, down):
        self.emit((EV_KEY, code, 1 if down else 0))

    def wheel(self, notches=0, hi_res=None, horizontal=False):
        """`notches` whole detents, or `hi_res` 120ths (a free-spinning wheel)."""
        hi = hi_res if hi_res is not None else notches * 120
        lo_code, hi_code = (REL_HWHEEL, REL_HWHEEL_HI_RES) if horizontal else (REL_WHEEL, REL_WHEEL_HI_RES)
        events = [(EV_REL, hi_code, hi)]
        if hi % 120 == 0:
            events.append((EV_REL, lo_code, hi // 120))
        self.emit(*events)


KEY_LEFTCTRL, KEY_LEFTSHIFT, KEY_LEFTALT = 29, 42, 56


class Keyboard(Device):
    """Modifier keys only."""

    def __init__(self):
        super().__init__("noshape-nav keyboard", keys=(KEY_LEFTCTRL, KEY_LEFTSHIFT, KEY_LEFTALT), product=0x40)

    def key(self, code, down):
        self.emit((EV_KEY, code, 1 if down else 0))


class MultiTouch(Device):
    """Shared multi-touch protocol B handling (slots + tracking ids)."""

    TOOLS = {1: BTN_TOOL_FINGER, 2: BTN_TOOL_DOUBLETAP, 3: BTN_TOOL_TRIPLETAP, 4: BTN_TOOL_QUADTAP}

    def __init__(self, name, size, res, props, product):
        self.size = size
        w, h = size
        axes = {ABS_X: (0, w, res), ABS_Y: (0, h, res), ABS_MT_SLOT: (0, 9, 0), ABS_MT_TRACKING_ID: (0, 65535, 0),
                ABS_MT_POSITION_X: (0, w, res), ABS_MT_POSITION_Y: (0, h, res)}
        keys = (BTN_TOUCH,) + tuple(self.TOOLS.values()) + ((BTN_LEFT,) if INPUT_PROP_POINTER in props else ())
        super().__init__(name, keys=keys, abs_axes=axes, props=props, product=product)
        self.down = {}  # slot -> (x, y)
        self.next_id = 1

    def frame(self, fingers):
        """Move to the finger set `fingers` {slot: (x, y)}: lifts, lands, moves."""
        ev = []
        for slot in sorted(set(self.down) | set(fingers)):
            ev.append((EV_ABS, ABS_MT_SLOT, slot))
            if slot not in fingers:
                ev.append((EV_ABS, ABS_MT_TRACKING_ID, -1))
                continue
            if slot not in self.down:
                ev.append((EV_ABS, ABS_MT_TRACKING_ID, self.next_id))
                self.next_id += 1
            x, y = (int(round(c)) for c in fingers[slot])
            ev += [(EV_ABS, ABS_MT_POSITION_X, x), (EV_ABS, ABS_MT_POSITION_Y, y)]
        if fingers:
            first = fingers[min(fingers)]
            ev += [(EV_ABS, ABS_X, int(round(first[0]))), (EV_ABS, ABS_Y, int(round(first[1])))]
        before, after = len(self.down), len(fingers)
        if before != after:
            if (before == 0) != (after == 0):
                ev.append((EV_KEY, BTN_TOUCH, 1 if after else 0))
            if before in self.TOOLS:
                ev.append((EV_KEY, self.TOOLS[before], 0))
            if after in self.TOOLS:
                ev.append((EV_KEY, self.TOOLS[after], 1))
        self.emit(*ev)
        self.down = dict(fingers)

    def gesture(self, start, end, steps, interval=0.012, hold=0.05):
        """Land the fingers at `start`, move them linearly to `end` in `steps`
        frames `interval` s apart, wait `hold` s, lift them."""
        self.frame(start)
        time.sleep(interval)
        for i in range(1, steps + 1):
            t = i / steps
            self.frame({k: (start[k][0] + (end[k][0] - start[k][0]) * t, start[k][1] + (end[k][1] - start[k][1]) * t) for k in start})
            time.sleep(interval)
        time.sleep(hold)
        self.frame({})


class Touchpad(MultiTouch):
    """A 100 × 65 mm clickpad, 40 units per mm (a typical laptop touchpad)."""

    def __init__(self):
        super().__init__("noshape-nav touchpad", (4000, 2600), 40, (INPUT_PROP_POINTER, INPUT_PROP_BUTTONPAD), 0x20)


class Touchscreen(MultiTouch):
    """A direct touch screen covering the output (`size` in its own units)."""

    def __init__(self, size=(16000, 10000)):
        super().__init__("noshape-nav touchscreen", size, 50, (INPUT_PROP_DIRECT,), 0x30)


assert ctypes.sizeof(ctypes.c_long) == 8, "input_event layout assumes a 64-bit kernel"
