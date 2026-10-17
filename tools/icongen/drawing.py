from base import *
from assembly import brackets

LPROF = [(0, 0), (6, 0), (6, 3), (3, 3), (3, 9), (0, 9)]


def lshape(x, y, s=1.0):
    return [(x + px * s, y + py * s) for px, py in LPROF]


def hatch(clip_d, x0, y0, x1, y1, step=3, cid="s"):
    d = []
    k = x0 - (y1 - y0)
    while k < x1:
        d.append(M([(k, y1), (k + (y1 - y0), y0)]))
        k += step
    return (f'<defs><clipPath id="{cid}"><path d="{clip_d}"/></clipPath></defs>'
            f'<g clip-path="url(#{cid})">' + P(" ".join(d), stroke_width="1") + "</g>")


@icon("drawing")
def _():
    out = rect(2.5, 4, 19, 16, 1.5)
    out += line((14, 20), (14, 15.5), (21.5, 15.5))
    l = lshape(5.5, 7, 1.0)
    out += fpoly(*l) + poly(*l)
    out += rect(14, 7, 4.5, 4.5, 0.5)
    return out


@icon("base-view")
def _():
    l = lshape(7, 6.5, 1.35)
    l = [(7 + px * 1.4, 5.7 + py * 1.4) for px, py in LPROF]
    return brackets(3, 3, 21, 21, 3.5) + fpoly(*l) + poly(*l)


@icon("projected-view")
def _():
    l = [(2.5 + px * 1.15, 6.8 + py * 1.15) for px, py in LPROF]
    out = fpoly(*l) + poly(*l)
    out += rect(15, 6.8, 6.9, 10.35, 0.3)
    out += dline((15, 10.25), (21.9, 10.25))
    out += arrow((10, 12), (13.4, 12), s=1.8)
    return out


@icon("auxiliary-view")
def _():
    base = [(3, 21), (3, 12), (8, 12), (12.5, 16.5), (12.5, 21)]
    out = fpoly(*base) + poly(*base)
    g = rect(-4.5, -2, 9, 4, 0.3)
    out += f'<g transform="translate(17.6 6.4) rotate(-45)">{g}</g>'
    out += arrow((11, 13.5), (13.9, 10.6), s=1.8)
    return out


@icon("section-view")
def _():
    sh = [(3.5, 4.5), (9, 4.5), (9, 13.5), (15, 13.5), (15, 4.5), (20.5, 4.5), (20.5, 19.5), (3.5, 19.5)]
    d = M(sh, True)
    return hatch(d, 3.5, 4.5, 20.5, 19.5, 3.2) + P(d)


@icon("detail-view")
def _():
    out = line((2, 14), (8, 14), (8, 21))
    out += circle(7, 15, 3.6, stroke_dasharray=DASH)
    out += circle(15.5, 8.5, 6.5)
    out += line((11.5, 7.5), (16.5, 7.5), (16.5, 14.8))
    out += line((9.5, 12.5), (10.8, 11.5), stroke_width="1.2")
    return out


@icon("broken-view")
def _():
    l = "M2.5 7.5 H10 Q8 9.75 10 12 Q12 14.25 10 16.5 H2.5 Z"
    r = "M21.5 7.5 H14 Q12 9.75 14 12 Q16 14.25 14 16.5 H21.5 Z"
    return fpath(l) + P(l) + fpath(r) + P(r)


@icon("crop-view")
def _():
    out = P("M6.5 2.5 V17.5 H21.5") + P("M2.5 6.5 H17.5 V21.5")
    out += frect(6.5, 6.5, 11, 11)
    return out


@icon("linear-dim")
def _():
    out = line((4, 21), (4, 8), stroke_width="1.2") + line((20, 21), (20, 8), stroke_width="1.2")
    out += arrow((12, 11), (4.4, 11), s=2.2) + arrow((12, 11), (19.6, 11), s=2.2)
    out += line((9, 6), (15, 6))
    out += line((2.5, 21), (21.5, 21))
    return out


@icon("angular-dim")
def _():
    v = (3.5, 20)
    out = line(v, (21.5, 20)) + line(v, (13.5, 4))
    ang = math.degrees(math.atan2(4 - 20, 13.5 - 3.5))
    out += arcarrow(3.5, 20, 12, 12, ang + 4, -3, s=2.2, both=True)
    return out


@icon("radial-dim")
def _():
    c = (10.5, 13.5)
    out = circle(*c, 7.5) + dot(*c, 1.4)
    e = polar(c, 7.5, -45)
    out += arrow(c, e, s=2.2) + line(e, (20.5, 4))
    return out


@icon("diameter-dim")
def _():
    c = (12, 12)
    out = circle(*c, 8)
    out += arrow(c, polar(c, 8, -45), s=2.2) + arrow(c, polar(c, 8, 135), s=2.2)
    out += line(polar(c, 8, -45), (21.5, 2.5))
    return out


@icon("ordinate-dim")
def _():
    out = line((3, 20.5), (21, 20.5))
    out += dot(4.5, 20.5, 1.6)
    for x, y in ((4.5, 5), (11, 9), (18, 13)):
        out += line((x, 19), (x, y), stroke_width="1.2") + line((x - 1.8, y), (x + 1.8, y))
    return out


@icon("note")
def _():
    out = rect(8.5, 3, 13, 10, 1.5)
    out += line((11.5, 6.5), (18.5, 6.5)) + line((11.5, 9.5), (16, 9.5))
    out += line((8.5, 11), (3.5, 20)) + dot(3.5, 20, 1.6)
    return out


@icon("balloon")
def _():
    out = circle(15, 9, 6)
    out += line((15, 6.5), (15, 11.5)) + line((14, 7.4), (15, 6.5))
    out += line((10.8, 13.2), (4, 20)) + dot(4, 20, 1.7)
    return out


@icon("centermark")
def _():
    out = circle(12, 12, 6.5)
    out += line((10, 12), (14, 12)) + line((12, 10), (12, 14))
    for a, b in (((12, 2), (12, 7.5)), ((12, 16.5), (12, 22)), ((2, 12), (7.5, 12)), ((16.5, 12), (22, 12))):
        out += line(a, b, stroke_width="1.2")
    return out


@icon("centerline")
def _():
    return line((4, 6), (20, 6)) + line((4, 18), (20, 18)) + ddline((1.5, 12), (22.5, 12))


@icon("gdt")
def _():
    out = rect(2, 7, 20, 10, 1)
    out += line((8.5, 7), (8.5, 17)) + line((15.5, 7), (15.5, 17))
    out += circle(5.25, 12, 2.1, stroke_width="1.2") + line((5.25, 9), (5.25, 15), stroke_width="1.2") + line((2.25, 12), (8.25, 12), stroke_width="1.2")
    out += line((10.5, 12), (13.5, 12))
    out += line((17, 15), (18.75, 9), (20.5, 15)) + line((17.6, 13), (19.9, 13), stroke_width="1.2")
    return out


@icon("surface-finish")
def _():
    return P("M3.5 12 L8 20 L15 4 H21.5") + line((2.5, 20.5), (13.5, 20.5)) if False else P("M3.5 11.5 L8.5 20 L15.5 4 H21.5")


@icon("datum")
def _():
    out = rect(7.5, 2.5, 9, 8, 1)
    out += line((10, 9), (12, 4.5), (14, 9)) + line((10.8, 7.4), (13.2, 7.4), stroke_width="1.2")
    out += line((12, 10.5), (12, 15))
    out += poly((12, 15), (16, 20.5), (8, 20.5), fill="black")
    out += line((3, 20.5), (21, 20.5))
    return out


def _table(first_col_glyph=None):
    out = frect(3, 3, 18, 5, 1.5, op="0.5") + rect(3, 3, 18, 18, 1.5)
    out += line((3, 8), (21, 8)) + line((3, 12.3), (21, 12.3)) + line((3, 16.6), (21, 16.6))
    out += line((9, 8), (9, 21))
    return out


@icon("bom")
def _():
    return _table() + line((13, 8), (13, 21), stroke_width="1.2")


@icon("hole-table")
def _():
    out = frect(3, 3, 18, 5, 1.5, op="0.5") + rect(3, 3, 18, 18, 1.5)
    out += line((3, 8), (21, 8)) + line((10, 8), (10, 21))
    for y in (11, 14.5, 18):
        out += circle(6.5, y, 1.5, stroke_width="1.2") + line((12.5, y), (18.5, y))
    return out


@icon("title-block")
def _():
    out = rect(3, 3, 18, 18, 1.5)
    out += frect(10.5, 14, 10.5, 7, op="0.35")
    out += line((10.5, 21), (10.5, 14), (21, 14)) + line((10.5, 17.5), (21, 17.5)) + line((15.5, 17.5), (15.5, 21))
    return out


@icon("sheet")
def _():
    out = rect(2.5, 4, 19, 16, 1.5) + rect(5, 6.5, 14, 11, 0.3, stroke_width="1.2")
    for a, b in (((12, 4), (12, 6.5)), ((12, 17.5), (12, 20)), ((2.5, 12), (5, 12)), ((19, 12), (21.5, 12))):
        out += line(a, b, stroke_width="1.2")
    return out
