from base import *
from constraints import ground


def brackets(x0, y0, x1, y1, s=3.5):
    return (line((x0, y0 + s), (x0, y0), (x0 + s, y0)) + line((x1 - s, y0), (x1, y0), (x1, y0 + s)) +
            line((x1, y1 - s), (x1, y1), (x1 - s, y1)) + line((x0 + s, y1), (x0, y1), (x0, y1 - s)))


@icon("insert")
def _():
    s, _ = cube(9.5, 9.5, 7.2)
    return s + badge_plus(18.5, 18.5)


@icon("instance")
def _():
    s, _ = cube(12, 12, 6.5, fill=("top",))
    return s + brackets(3, 3, 21, 21)


@icon("fastened")
def _():
    out = disc(12, 15.5, 7, 2.8, fill=True)
    out += disc(12, 10.5, 7, 2.8)
    out += rect(9.5, 3, 5, 3, 0.8) + line((12, 6), (12, 21))
    out += line((10, 21), (14, 21))
    return out


@icon("revolute")
def _():
    out = ddline((12, 2), (12, 22))
    out += fellipse(12, 13, 6.5, 2.6) + ellipse(12, 13, 6.5, 2.6) + ellipse(12, 8, 6.5, 2.6)
    out += arcarrow(12, 18, 8.5, 3, 165, 15, s=2.4)
    return out


@icon("slider")
def _():
    out = line((2.5, 18), (21.5, 18)) + line((2.5, 21), (21.5, 21))
    out += frect(8, 10.5, 8, 7.5, 1) + rect(8, 10.5, 8, 7.5, 1)
    out += arrow((12, 6), (21, 6), s=2.4) + arrow((12, 6), (3, 6), s=2.4)
    return out


@icon("planar")
def _():
    L, B, R, F = (1.5, 14.5), (12, 8.5), (22.5, 14.5), (12, 20.5)
    out = fpoly(L, B, R, F, op="0.2") + poly(L, B, R, F)
    c = (12, 14.5)
    u = (C30 * 7.5, 0.5 * 7.5)
    v = (-C30 * 7.5, 0.5 * 7.5)
    out += arrow(c, add(c, u), s=2.2) + arrow(c, sub(c, u), s=2.2)
    out += arrow(c, add(c, v), s=2.2) + arrow(c, sub(c, v), s=2.2)
    out += dot(12, 14.5, 1.6)
    return out


@icon("cylindrical")
def _():
    out = ellipse(12, 4.5, 3, 1.2) + line((9, 4.5), (9, 21.5)) + line((15, 4.5), (15, 21.5))
    out += fellipse(12, 14, 6.5, 2.4) + ellipse(12, 14, 6.5, 2.4)
    out += arcarrow(12, 14, 9.5, 3.8, 165, 15, s=2.2)
    out += arrow((20, 6), (20, 2.2), s=2) + arrow((20, 6), (20, 9.8), s=2)
    return out


@icon("pin-slot")
def _():
    out = P("M6.5 8 H17.5 A4 4 0 0 1 17.5 16 H6.5 A4 4 0 0 1 6.5 8 Z")
    out += fcircle(6.8, 12, 2.4, op="0.6") + circle(6.8, 12, 2.4)
    out += arrow((11, 12), (18.5, 12), s=2.2)
    out += arcarrow(6.8, 12, 5.5, 5.5, 150, 230, s=1.8) if False else ""
    return out


@icon("ball")
def _():
    out = circle(12, 11, 5.8) + arc(12, 11, 5.8, 2, 0, 180)
    out += fpath("M6.2 11 A5.8 5.8 0 0 0 17.8 11 A5.8 2 0 0 1 6.2 11 Z")
    out += arc(12, 11, 8.6, 8.6, 20, 160)
    out += arcarrow(12, 11, 8.6, 8.6, 220, 320, s=2, both=True)
    return out


@icon("parallel-mate")
def _():
    lo = [(3, 16.5), (12, 12), (21, 16.5), (12, 21)]
    hi = [(3, 7.5), (12, 3), (21, 7.5), (12, 12)]
    out = fpoly(*lo) + poly(*lo) + poly(*hi)
    out += dot(12, 7.5, 1.4) + dot(12, 16.5, 1.4)
    return out


@icon("tangent-mate")
def _():
    L, B, R, F = (1.5, 17), (12, 12), (22.5, 17), (12, 22)
    out = fpoly(L, B, R, F, op="0.2") + poly(L, B, R, F)
    out += circle(12, 10, 7) + arc(12, 10, 7, 2.4, 0, 180)
    out += dot(12, 17, 1.8)
    return out


@icon("width-mate")
def _():
    out = line((3.5, 4), (3.5, 20)) + line((20.5, 4), (20.5, 20))
    out += frect(9, 6.5, 6, 11) + rect(9, 6.5, 6, 11)
    out += arrow((6, 12), (8.6, 12), s=1.8) + arrow((6, 12), (3.9, 12), s=1.8)
    out += arrow((18, 12), (15.4, 12), s=1.8) + arrow((18, 12), (20.1, 12), s=1.8)
    return out


@icon("gear-relation")
def _():
    out = gear(8.2, 8.8, 6, 8, 1.7, 0) + circle(8.2, 8.8, 1.6)
    out += gear(16.4, 16.2, 5, 7, 1.6, 13) + circle(16.4, 16.2, 1.4)
    return out


@icon("rack-pinion")
def _():
    out = gear(12, 8, 6, 9, 1.7, -90 + 10) + circle(12, 8, 1.6)
    d = "M2 21 V17"
    x = 2
    while x < 21:
        d += f" H{f(x+0.9)} L{f(x+1.5)} 15 H{f(x+2.6)} L{f(x+3.2)} 17"
        x += 3.2
    d += " H22 V21 Z"
    out += P(d)
    return out


@icon("screw-relation")
def _():
    out = rect(7.5, 3, 9, 3.5, 1)
    out += line((9.5, 6.5), (9.5, 18.5), (12, 21.5), (14.5, 18.5), (14.5, 6.5))
    for y in (10, 13.5, 17):
        out += line((9.5, y), (14.5, y - 2))
    out += arrow((19.5, 8), (19.5, 19), s=2.2)
    out += arcarrow(12, 4.7, 7.5, 2.4, 160, 20, s=2) if False else ""
    out += P("M4.5 9 A2.5 2.5 0 1 1 4.5 13") if False else ""
    return out


@icon("linear-relation")
def _():
    out = rect(4, 4, 5, 4, 0.8) + arrow((11, 6), (20.5, 6), s=2)
    out += rect(4, 16, 5, 4, 0.8) + arrow((11, 18), (16, 18), s=2)
    out += dline((6.5, 8.5), (6.5, 15.5))
    return out


@icon("group")
def _():
    out = rect(2.5, 2.5, 19, 19, 3.5, stroke_dasharray=DASH)
    s1, _ = cube(8.6, 10.4, 3.8, fill=("top",))
    s2, _ = cube(15.4, 13.6, 3.8)
    return out + s1 + s2


@icon("replicate")
def _():
    s, _ = cube(7, 7.5, 5, fill=("top", "left", "right"))
    out = s
    for c in ((17.5, 8), (17.5, 14.5), (11, 19)):
        out += ellipse(c[0], c[1], 3.4, 1.4) + dot(*c, 0.9)
    out += arrow((11.5, 11), (14.5, 12.5), s=1.8)
    return out


@icon("explode")
def _():
    o = center_o(12, 12, 13, 12, 12)
    base = Box(o, 12, 12, 3)
    top = Box(o, 6, 6, 4, x0=3, y0=3, z0=9)
    out = base.edges() + fpoly(*top.top()) + top.edges()
    p = lambda x, y, z: iso(o, x, y, z)
    out += dline(p(9, 3, 9), p(9, 3, 3.6)) + dline(p(3, 9, 9), p(3, 9, 3.6)) + dline(p(9, 9, 9), p(9, 9, 3.6))
    return out


@icon("named-position")
def _():
    s, _ = cube(9, 14, 6.8)
    out = s + line((18, 21), (18, 3))
    out += poly((18, 3), (22, 5.2), (18, 7.4), fill="black", fill_opacity=HLOP)
    return out


@icon("animate")
def _():
    out = arcarrow(12, 12, 9, 9, -60, 240, s=2.6)
    out += poly((10, 8), (16.2, 12), (10, 16), fill="black", fill_opacity="0.6")
    return out


@icon("interference")
def _():
    o1 = center_o(7.5, 7.5, 7.5, 8.6, 12)
    o2 = center_o(7.5, 7.5, 7.5, 15.4, 12)
    b1, b2 = Box(o1, 7.5, 7.5, 7.5), Box(o2, 7.5, 7.5, 7.5)
    c1 = M(b1.outline(), True)
    c2 = M(b2.outline(), True)
    out = f'<defs><clipPath id="i"><path d="{c1}"/></clipPath></defs>'
    out += f'<g clip-path="url(#i)">{fpath(c2, op="0.6")}</g>'
    out += b1.edges() + b2.edges()
    return out


@icon("fix-instance")
def _():
    s, _ = cube(12, 9, 7.5)
    return s + ground(19, 4, 20)


@icon("mate-connector-assembly")
def _():
    s, b = cube(12, 15, 8.5)
    c = b.center("top")
    out = s + fellipse(c[0], c[1], 4.4, 2, op="0.6") + ellipse(c[0], c[1], 4.4, 2) + dot(*c, 1.2)
    out += arrow(c, (c[0], 1.8), s=2.4)
    return out
