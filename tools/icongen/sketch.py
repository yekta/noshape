from base import *

D = 1.6  # sketch point dot radius


def dots(*ps, r=D):
    return "".join(dot(*p, r) for p in ps)


def ring(c, r=2.6):
    return circle(c[0], c[1], r)


@icon("line")
def _():
    return line((4, 19), (20, 5)) + dots((4, 19), (20, 5))


@icon("polyline")
def _():
    ps = [(3, 18), (8.5, 6.5), (14.5, 16), (21, 5)]
    return line(*ps) + dots(*ps)


@icon("midpoint-line")
def _():
    return line((3, 18.5), (21, 5.5)) + dots((3, 18.5), (21, 5.5)) + ring((12, 12), 2.8) + dot(12, 12, 1.1)


@icon("corner-rectangle")
def _():
    return rect(4, 6, 16, 12) + dots((4, 18), (20, 6), r=2)


@icon("center-rectangle")
def _():
    return rect(3.5, 5.5, 17, 13) + dline((12, 12), (20.5, 5.5)) + dots((12, 12), (20.5, 5.5), r=1.8)


def _rot_rect():
    A = (4.6, 11.8)
    u = (0.866, -0.5)
    n = (0.5, 0.866)
    Bp = add(A, mul(u, 12.5))
    Cc = add(Bp, mul(n, 8))
    Dd = add(A, mul(n, 8))
    return A, Bp, Cc, Dd, u


@icon("three-point-rectangle")
def _():
    A, Bp, Cc, Dd, u = _rot_rect()
    return poly(A, Bp, Cc, Dd) + dots(A, Bp, Cc, r=1.9)


@icon("aligned-rectangle")
def _():
    A, Bp, Cc, Dd, u = _rot_rect()
    out = poly(A, Bp, Cc, Dd)
    out += dline(sub(A, mul(u, 3)), A) + dline(Bp, add(Bp, mul(u, 3)))
    out += fpoly(A, Bp, Cc, Dd, op="0.2")
    return out


def _ngon(c, r, n=6, phase=-90):
    return [polar(c, r, phase + 360 * i / n) for i in range(n)]


@icon("polygon-inscribed")
def _():
    return circle(12, 12, 9, stroke_dasharray=DASH) + poly(*_ngon((12, 12), 9)) + dot(12, 12, 1.4)


@icon("polygon-circumscribed")
def _():
    return circle(12, 12, 7.2, stroke_dasharray=DASH) + poly(*_ngon((12, 12), 7.2 / C30, phase=0)) + dot(12, 12, 1.4)


@icon("circle")
def _():
    return circle(12, 12, 8.5) + dot(12, 12, D) + dline((12, 12), polar((12, 12), 8.5, -40))


@icon("three-point-circle")
def _():
    return circle(12, 12, 8.5) + dots(*[polar((12, 12), 8.5, a) for a in (150, 270, 30)], r=1.9)


@icon("center-point-arc")
def _():
    c = (12, 16)
    out = arc(12, 16, 9, 9, 195, 345)
    out += dline(polar(c, 9, 195), c, polar(c, 9, 345))
    return out + dot(*c, D) + dots(polar(c, 9, 195), polar(c, 9, 345))


@icon("three-point-arc")
def _():
    c = (12, 17)
    return arc(12, 17, 9.5, 9.5, 190, 350) + dots(*[polar(c, 9.5, a) for a in (190, 270, 350)], r=1.9)


@icon("tangent-arc")
def _():
    out = line((3, 18), (11, 18))
    out += P("M11 18 A6.5 6.5 0 0 0 15.6 6.9")
    return out + dots((3, 18), (15.6, 6.9)) + ring((11, 18), 2.4)


@icon("ellipse")
def _():
    return ellipse(12, 12, 9.5, 6) + dot(12, 12, D) + dline((12, 12), (21.5, 12))


@icon("elliptical-arc")
def _():
    out = arc(12, 13, 9.5, 6.5, 180, 360)
    out += arc(12, 13, 9.5, 6.5, 0, 180, stroke_dasharray=DASH)
    return out + dots((2.5, 13), (21.5, 13)) + dot(12, 13, 1.3)


@icon("conic")
def _():
    A, Bp, apex = (3, 19), (21, 19), (12, 4)
    out = dline(A, apex, Bp)
    out += P(f"M{pt(A)} Q{pt(apex)} {pt(Bp)}")
    out += dots(A, Bp) + rect(10.6, 2.6, 2.8, 2.8, stroke_width="1.2") + dot(12, 11.5, 1.5)
    return out


@icon("spline")
def _():
    ps = [(3, 17), (8, 7.5), (15, 16.5), (21, 6.5)]
    return P(smooth(ps)) + dots(*ps)


@icon("bezier")
def _():
    a = ((3, 18), (5, 4), (19, 20), (21, 6))
    out = P(C(*a))
    out += line(a[0], a[1], stroke_width="1.2") + line(a[3], a[2], stroke_width="1.2")
    out += circle(*a[1], 1.6, stroke_width="1.2") + circle(*a[2], 1.6, stroke_width="1.2")
    out += rect(a[0][0] - 1.6, a[0][1] - 1.6, 3.2, 3.2, fill="black", stroke="none")
    out += rect(a[3][0] - 1.6, a[3][1] - 1.6, 3.2, 3.2, fill="black", stroke="none")
    return out


@icon("slot")
def _():
    g = ("M7.5 7 H16.5 A5 5 0 0 1 16.5 17 H7.5 A5 5 0 0 1 7.5 7 Z")
    out = P(g) + dline((7.5, 12), (16.5, 12)) + dots((7.5, 12), (16.5, 12), r=1.4)
    return f'<g transform="rotate(-30 12 12)">{out}</g>'


@icon("point-sketch")
def _():
    out = dot(12, 12, 2.3)
    for d in ((1, 0), (-1, 0), (0, 1), (0, -1)):
        out += line(add((12, 12), mul(d, 5)), add((12, 12), mul(d, 9.5)))
    return out


@icon("construction")
def _():
    out = circle(13.5, 10.5, 6.5, stroke_dasharray=DASH)
    out += dline((2.5, 21.5), (21.5, 2.5))
    return out + dot(13.5, 10.5, 1.4)


@icon("text")
def _():
    return P("M5 7 V4.5 H19 V7 M12 4.5 V19.5 M9 19.5 H15")


@icon("image")
def _():
    out = rect(3, 4.5, 18, 15, 2)
    out += P("M3.5 17.5 L9 12 L13 16 L15.5 13.5 L20.5 18.5")
    out += circle(15.5, 9, 1.8)
    return out


@icon("sketch-fillet")
def _():
    out = fpath("M4.5 13 V19.5 H11 A6.5 6.5 0 0 1 4.5 13 Z")
    out += line((4.5, 3.5), (4.5, 13)) + line((11, 19.5), (20.5, 19.5))
    out += P("M4.5 13 A6.5 6.5 0 0 0 11 19.5")
    out += dline((4.5, 13), (4.5, 19.5), (11, 19.5))
    return out + dots((4.5, 13), (11, 19.5), r=1.4)


@icon("sketch-chamfer")
def _():
    out = fpoly((4.5, 12.5), (4.5, 19.5), (11.5, 19.5))
    out += line((4.5, 3.5), (4.5, 12.5), (11.5, 19.5), (20.5, 19.5))
    out += dline((4.5, 12.5), (4.5, 19.5), (11.5, 19.5))
    return out + dots((4.5, 12.5), (11.5, 19.5), r=1.4)


@icon("trim")
def _():
    out = line((8, 3.5), (8, 20.5)) + line((16, 3.5), (16, 20.5))
    out += line((3, 12), (8, 12)) + line((16, 12), (21, 12))
    out += dline((8, 12), (16, 12))
    out += line((10.5, 7.5), (13.5, 10.5), stroke_width="1.3") + line((13.5, 7.5), (10.5, 10.5), stroke_width="1.3")
    return out


@icon("extend")
def _():
    out = line((19.5, 3), (19.5, 21))
    out += line((3.5, 17.5), (10.5, 13.5)) + dot(3.5, 17.5, D)
    out += dline((10.5, 13.5), (17.6, 9.4)) + head((19, 8.6), (0.866, -0.5), 2.6)
    return out


@icon("split-sketch")
def _():
    out = line((3, 18.5), (10.4, 13.1)) + line((13.6, 10.9), (21, 5.5))
    out += dots((3, 18.5), (21, 5.5)) + dots((10.4, 13.1), (13.6, 10.9), r=1.4)
    out += line((8.5, 8), (15.5, 16), stroke_width="1.3")
    return out


@icon("offset")
def _():
    out = rect(7.5, 7.5, 9, 9, 1.5, stroke_dasharray=DASH)
    out += rect(3, 3, 18, 18, 4.5)
    out += arrow((16.5, 16.5), (19.4, 19.4), s=2)
    return out


@icon("sketch-mirror")
def _():
    tri = [(3.5, 19), (9.5, 19), (9.5, 6)]
    out = fpoly(*tri) + poly(*tri)
    out += f'<g transform="matrix(-1 0 0 1 24 0)">{poly(*tri)}</g>'
    return out + ddline((12, 2), (12, 22))


@icon("sketch-linear-pattern")
def _():
    out = frect(2.5, 6, 5, 5) + rect(2.5, 6, 5, 5) + rect(9.5, 6, 5, 5) + rect(16.5, 6, 5, 5)
    return out + arrow((3, 17), (21, 17), s=2.4)


@icon("sketch-circular-pattern")
def _():
    out = ""
    for i, a in enumerate((-90, 0, 90, 180)):
        c = polar((12, 12), 8, a)
        if i == 0:
            out += fcircle(*c, 2.4)
        out += circle(*c, 2.4)
    out += arcarrow(12, 12, 8, 8, -70, -20, s=2) + arcarrow(12, 12, 8, 8, 20, 70, s=2)
    out += dot(12, 12, 1.4)
    return out


@icon("transform-sketch")
def _():
    c = (12, 12)
    out = ""
    for d in ((1, 0), (-1, 0), (0, 1), (0, -1)):
        out += arrow(add(c, mul(d, 4.5)), add(c, mul(d, 10)), s=2.4)
    out += frect(9.5, 9.5, 5, 5) + rect(9.5, 9.5, 5, 5)
    return out


@icon("use")
def _():
    L, B, R, F = (2, 16.5), (12, 11.5), (22, 16.5), (12, 21.5)
    out = poly(L, B, R, F)
    s, b = cube(12, 6.5, 5)
    out += s
    fp = [(7.67, 16.5), (12, 14), (16.33, 16.5), (12, 19)]
    out += fpoly(*fp, op="0.6") + poly(*fp)
    out += dline((7.67, 9), (7.67, 16.5)) + dline((16.33, 9), (16.33, 16.5)) + dline((12, 11.5), (12, 14))
    return out


@icon("intersection")
def _():
    L, B, R, F = (1.5, 13), (12, 7.5), (22.5, 13), (12, 18.5)
    out = fpoly(L, B, R, F, op="0.15") + poly(L, B, R, F)
    out += ellipse(12, 13, 5, 2) + fellipse(12, 13, 5, 2, op="0.7")
    out += ellipse(12, 4, 5, 2) + line((7, 4), (7, 13)) + line((17, 4), (17, 13))
    out += dline((7, 13), (7, 20)) + dline((17, 13), (17, 20))
    out += arc(12, 20, 5, 2, 0, 180, stroke_dasharray=DASH)
    return out


@icon("dimension")
def _():
    A, Bp = (4, 20), (20, 8)
    out = line(A, Bp) + dots(A, Bp, r=1.4)
    n = (-0.6, -0.8)
    a2, b2 = add(A, mul(n, 7)), add(Bp, mul(n, 7))
    out += line(add(A, mul(n, 2)), add(A, mul(n, 8.5)), stroke_width="1.2")
    out += line(add(Bp, mul(n, 2)), add(Bp, mul(n, 8.5)), stroke_width="1.2")
    out += arrow(lerp(a2, b2, 0.5), a2, s=2.2) + arrow(lerp(a2, b2, 0.5), b2, s=2.2)
    return out
