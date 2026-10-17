from base import *


@icon("coincident")
def _():
    out = line((3, 19), (12, 12)) + line((12, 12), (21, 16))
    out += circle(12, 12, 3.6) + dot(12, 12, 1.6)
    out += dot(3, 19, 1.3) + dot(21, 16, 1.3)
    return out


@icon("concentric")
def _():
    return circle(12, 12, 9) + circle(12, 12, 4.6) + dot(12, 12, 1.6)


@icon("parallel")
def _():
    return line((5, 19.5), (12.5, 4.5)) + line((11.5, 19.5), (19, 4.5))


@icon("tangent")
def _():
    return circle(12, 14, 6.5) + line((2.5, 7.5), (21.5, 7.5)) + dot(12, 7.5, 1.8)


@icon("horizontal")
def _():
    out = line((3.5, 10), (20.5, 10)) + dot(3.5, 10, 1.8) + dot(20.5, 10, 1.8)
    out += arrow((12, 17), (20, 17), s=2.2) + arrow((12, 17), (4, 17), s=2.2)
    return out


@icon("vertical")
def _():
    out = line((10, 3.5), (10, 20.5)) + dot(10, 3.5, 1.8) + dot(10, 20.5, 1.8)
    out += arrow((17, 12), (17, 4), s=2.2) + arrow((17, 12), (17, 20), s=2.2)
    return out


@icon("midpoint")
def _():
    out = line((3, 14), (21, 14)) + dot(3, 14, 1.3) + dot(21, 14, 1.3)
    out += poly((12, 6.5), (15, 11), (9, 11), fill="black", fill_opacity=HLOP)
    out += dot(12, 14, 2)
    out += line((7.5, 12), (7.5, 16)) + line((16.5, 12), (16.5, 16))
    return out


@icon("normal")
def _():
    out = arc(12, 27, 15, 15, 215, 325)
    out += line((12, 12), (12, 2.5))
    out += line((12, 8.8), (15.2, 8.8), (15.2, 12.4))
    out += dot(12, 12, 1.5)
    return out


@icon("pierce")
def _():
    L, B, R, F = (2, 14), (12, 9.5), (22, 14), (12, 18.5)
    out = fpoly(L, B, R, F, op="0.2") + poly(L, B, R, F)
    out += line((12, 2.5), (12, 14)) + dline((12, 14), (12, 22))
    out += circle(12, 14, 2.6) + dot(12, 14, 1.2)
    return out


@icon("perpendicular")
def _():
    return line((3.5, 19.5), (20.5, 19.5)) + line((12, 19.5), (12, 3.5)) + line((12, 15.5), (16, 15.5), (16, 19.5))


@icon("equal")
def _():
    return line((5, 8.5), (19, 8.5)) + line((5, 15.5), (19, 15.5))


@icon("symmetric")
def _():
    out = ddline((12, 2.5), (12, 21.5))
    out += dot(4.5, 12, 2) + dot(19.5, 12, 2)
    out += arrow((9.5, 12), (6.5, 12), s=2) + arrow((14.5, 12), (17.5, 12), s=2)
    return out


def ground(y, x0=4, x1=20):
    out = line((x0, y), (x1, y))
    for x in range(int(x0) + 2, int(x1) + 1, 4):
        out += line((x, y), (x - 2.6, y + 3), stroke_width="1.3")
    return out


@icon("fix")
def _():
    out = circle(12, 6.5, 2.6)
    out += poly((12, 9.1), (6.5, 16.5), (17.5, 16.5))
    out += ground(16.5)
    return out


@icon("curvature")
def _():
    a = ((3, 18), (9, 4), (15, 18), (21, 7))
    out = P(C(*a))
    env = []
    for i in range(0, 11):
        t = 0.05 + 0.9 * i / 10
        p = bez(*a, t)
        d = norm(bezt(*a, t))
        n = (d[1], -d[0])
        k = 1.5 + 3.5 * abs(math.sin(math.pi * t * 1.6))
        q = add(p, mul(n, -k if t < 0.5 else k))
        q = add(p, mul(n, -k))
        env.append(q)
        out += line(p, q, stroke_width="1")
    out += P(smooth(env), stroke_width="1")
    return out


@icon("collinear")
def _():
    out = line((3, 19), (9, 14.5)) + line((15, 9.5), (21, 5))
    out += dline((9, 14.5), (15, 9.5))
    return out + dot(3, 19, 1.4) + dot(9, 14.5, 1.4) + dot(15, 9.5, 1.4) + dot(21, 5, 1.4)


@icon("driven-dimension")
def _():
    out = P("M5 4.5 Q1.5 12 5 19.5") + P("M19 4.5 Q22.5 12 19 19.5")
    out += line((8, 7), (8, 17), stroke_width="1.2") + line((16, 7), (16, 17), stroke_width="1.2")
    out += arrow((12, 12), (8.2, 12), s=2) + arrow((12, 12), (15.8, 12), s=2)
    return out
