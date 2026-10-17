from base import *
from part import folder_d
from assembly import brackets


def eye_glyph(cx, cy, w, h, pupil):
    d = (f"M{f(cx-w)} {f(cy)} Q{f(cx)} {f(cy-h*2)} {f(cx+w)} {f(cy)} "
         f"Q{f(cx)} {f(cy+h*2)} {f(cx-w)} {f(cy)} Z")
    return P(d) + circle(cx, cy, pupil)


def _face_inset(ps, k=0.62):
    c = (sum(q[0] for q in ps) / 4, sum(q[1] for q in ps) / 4)
    return [lerp(c, q, k) for q in ps]


@icon("view-cube")
def _():
    s, b = cube(12, 12, 9.5)
    out = s
    for face, op in (("top", "0.6"), ("left", None), ("right", None)):
        ps = _face_inset(getattr(b, face)())
        if op:
            out += fpoly(*ps, op=op)
        out += poly(*ps, stroke_width="1.2")
    return out


@icon("iso-view")
def _():
    s, b = cube(12, 12, 9.5)
    return s + circle(12, 12, 2.4, fill="black")


def _view_dir(face, ad):
    s, b = cube(12.5 if face != "right" else 11.5, 13.5, 7, fill=(face,))
    c = b.center(face)
    tip = add(c, mul(ad, 4.2)) if face != "top" else (c[0], c[1] - 4.6)
    tail = add(tip, mul(ad, 6))
    return s + arrow(tail, tip, s=2.4)


@icon("front-view")
def _():
    return _view_dir("left", (-C30, 0.5))


@icon("top-view")
def _():
    s, b = cube(12, 14.5, 7, fill=("top",))
    return s + arrow((12, 0.8), (12, 5.6), s=2.4)


@icon("right-view")
def _():
    return _view_dir("right", (C30, 0.5))


@icon("zoom-fit")
def _():
    out = brackets(3, 3, 21, 21, 4.5)
    out += rect(8, 8, 8, 8, 1.2) + frect(8, 8, 8, 8, 1.2)
    return out


@icon("zoom-window")
def _():
    out = circle(10, 10, 7) + line((15.2, 15.2), (21, 21))
    out += rect(7, 7.5, 6, 5, 0.5, stroke_width="1.2", stroke_dasharray="1 2")
    return out


@icon("section-plane")
def _():
    o = center_o(7, 11, 10, 12.5, 12)
    b = Box(o, 6, 11, 10)
    p = lambda x, y, z: iso(o, x, y, z)
    out = fpoly(*b.right(), op="0.5") + b.edges()
    e = 2.6
    pl = [p(6, -e, -e), p(6, 11 + e, -e), p(6, 11 + e, 10 + e), p(6, -e, 10 + e)]
    out += P(M([pl[1], pl[0], pl[3], pl[2]]) + " " + M([pl[1], p(6, 11, 0)]) + " " + M([pl[2], p(6, 11, 10)]), stroke_width="1.2")
    out += P(M([pl[1], pl[2]]), stroke_width="1.2")
    return out


@icon("measure")
def _():
    g = rect(-10, -3.8, 20, 7.6, 1.2)
    for i, x in enumerate(range(-7, 9, 3)):
        g += line((x, -3.8), (x, -1.2 if i % 2 else 0.3), stroke_width="1.2")
    return f'<g transform="translate(12 12) rotate(-45)">{g}</g>'


@icon("mass-properties")
def _():
    out = circle(12, 12, 8.5)
    out += P("M12 12 V3.5 A8.5 8.5 0 0 1 20.5 12 Z", fill="black", stroke="none")
    out += P("M12 12 V20.5 A8.5 8.5 0 0 1 3.5 12 Z", fill="black", stroke="none")
    out += line((12, 3.5), (12, 20.5)) + line((3.5, 12), (20.5, 12))
    return out


SHADES = (("top", "0.25"), ("left", "0.55"), ("right", "0.85"))


@icon("shaded")
def _():
    o = center_o(10, 10, 10)
    b = Box(o, 10, 10, 10)
    return "".join(fpoly(*getattr(b, fc)(), op=op) for fc, op in SHADES)


@icon("shaded-edges")
def _():
    s, b = cube(12, 12, 10, fill=SHADES)
    return s


@icon("wireframe")
def _():
    s, b = cube(12, 12, 10)
    return s + _hidden_solid(b)


def _hidden_solid(b):
    p = b.p
    h = p(b.x0, b.y0, b.z0)
    return line(p(b.x1, b.y0, b.z0), h, p(b.x0, b.y1, b.z0)) + line(h, p(b.x0, b.y0, b.z1))


@icon("hidden-lines")
def _():
    s, b = cube(12, 12, 10)
    return s + b.hidden()


@icon("perspective")
def _():
    F = [(3, 9), (13, 9), (13, 20), (3, 20)]
    Bk = [(14, 5), (18.5, 5), (18.5, 10.5), (14, 10.5)]
    out = poly(*F)
    out += line(F[0], Bk[0], Bk[1], Bk[2], F[2]) + line(F[1], Bk[1])
    out += dot(22, 2.5, 1.2) if False else ""
    out += dline(Bk[1], (21.5, 2)) + dline(Bk[0], (21.5, 2)) if False else ""
    return out


@icon("orthographic")
def _():
    F = [(3, 10), (13, 10), (13, 20), (3, 20)]
    k = (7, -6)
    Bk = [add(q, k) for q in F]
    out = poly(*F)
    out += line(F[0], Bk[0], Bk[1], Bk[2], F[2]) + line(F[1], Bk[1])
    return out


@icon("show")
def _():
    s, _ = cube(10, 9.5, 7.2)
    return s + eye_glyph(17.5, 18.5, 4.8, 2.4, 1.3)


@icon("hide")
def _():
    s, _ = cube(10, 9.5, 7.2, stroke_dasharray=DASH)
    return s + eye_glyph(17.5, 18.5, 4.8, 2.4, 1.3) + line((13.5, 22.5), (21.5, 14.5))


@icon("isolate")
def _():
    g1, _ = cube(4.4, 12, 3.4, stroke_dasharray="1 2", stroke_width="1.2")
    g2, _ = cube(19.6, 12, 3.4, stroke_dasharray="1 2", stroke_width="1.2")
    s, _ = cube(12, 12, 5.4, fill=("top", "left", "right"))
    return g1 + g2 + s


@icon("transparency")
def _():
    out = rect(3, 3, 12, 12, 1.5)
    out += frect(9, 9, 12, 12, 1.5) + rect(9, 9, 12, 12, 1.5)
    return out


@icon("appearance")
def _():
    d = ("M12 3 C6.5 3 3 7 3 11.5 C3 16.5 7 21 11.5 21 C13 21 13.5 20 13 18.8 "
         "C12.4 17.4 13.2 16 14.8 16 H17 C19.5 16 21 14.5 21 12 C21 7 17 3 12 3 Z")
    out = P(d)
    for c in ((7.6, 10.5), (11, 7), (15.5, 7.6), (8, 15)):
        out += dot(*c, 1.5)
    return out


@icon("material")
def _():
    out = circle(12, 12, 8.5)
    out += fpath("M19.4 7.8 A8.5 8.5 0 0 1 7.8 19.4 A10.5 10.5 0 0 0 19.4 7.8 Z", op="0.6")
    out += fpath("M19.4 7.8 A8.5 8.5 0 0 1 7.8 19.4 A13 13 0 0 0 19.4 7.8 Z", op="0.3")
    out += arc(12, 12, 5, 5, 200, 250, stroke_width="1.5")
    return out


@icon("part")
def _():
    s, _ = cube(12, 12, 9.5, fill=("top", "left", "right"))
    return s


@icon("surface-body")
def _():
    d, s = sheet(12, 12, w=9.5, h=5.5, amp=2, amp2=1.4)
    return fpath(d) + P(d)


@icon("wire-body")
def _():
    o = center_o(10, 10, 10)
    p = lambda x, y, z: iso(o, x, y, z)
    ps = [p(0, 10, 0), p(10, 10, 0), p(10, 10, 10), p(10, 0, 10)]
    d = (f"M{pt(ps[0])} L{pt(ps[1])} L{pt(ps[2])} Q{pt(p(10, 0, 10))} {pt(p(4, 0, 10))}")
    out = line(ps[0], ps[1]) + P(f"M{pt(ps[1])} C{pt(p(10, 10, 6))} {pt(p(10, 10, 10))} {pt(p(10, 5, 10))} L{pt(p(10, 0, 10))} Q{pt(p(4, -1, 10))} {pt(p(2, 2, 6))}")
    out = P(f"M{pt(ps[0])} L{pt(ps[1])} C{pt(p(10, 10, 7))} {pt(p(10, 10, 10))} {pt(p(10, 5, 10))} "
            f"C{pt(p(10, 0, 10))} {pt(p(5, 0, 10))} {pt(p(2, 0, 7))}")
    return out + dot(*ps[0], 1.8) + dot(*p(2, 0, 7), 1.8) + dot(*ps[1], 1.3)


@icon("part-studio")
def _():
    L, B, R, F = (2, 15.5), (12, 10), (22, 15.5), (12, 21)
    out = fpoly(L, B, R, F, op="0.2") + poly(L, B, R, F)
    s, _ = cube(12, 12.5, 6, fill=("top",))
    return out + s


@icon("assembly")
def _():
    o = center_o(12, 12, 11, 12, 12.5)
    b = Box(o, 12, 12, 4)
    out = b.edges()
    c = iso(o, 6, 6, 11)
    c0 = iso(o, 6, 6, 4)
    rx, ry = 3.4 * 1.2247, 3.4 * 0.7071
    out += fellipse(c[0], c[1], rx, ry) + ellipse(c[0], c[1], rx, ry)
    out += line((c[0] - rx, c[1]), (c0[0] - rx, c0[1])) + line((c[0] + rx, c[1]), (c0[0] + rx, c0[1]))
    out += arc(c0[0], c0[1], rx, ry, 0, 180)
    return out


@icon("version-graph")
def _():
    out = line((6, 19), (6, 5)) + P("M6 15 Q6 11 12 11 H15") + P("M6 9 Q6 6 10 6 H15") if False else ""
    out = line((5, 20), (5, 13.2)) + P("M5 13.2 Q5 8.5 9.8 8.5") + P("M5 13.2 Q5 18 9.8 18")
    out += line((14.2, 8.5), (16.8, 8.5)) + line((14.2, 18), (16.8, 18)) if False else ""
    out = (line((5, 21), (5, 3)) + P("M5 15 Q5 12 8 12 H15.8") + P("M5 9 Q5 5.5 9 5.5 H11.8"))
    out += P("M5 15 L5 15") if False else ""
    nodes = [(5, 3), (5, 21), (18, 12), (14, 5.5)]
    for c in nodes:
        out += circle(*c, 2.2) if False else ""
    out = ""
    out += line((5, 5.2), (5, 18.8))
    out += P("M5 15.5 Q5 12 8.5 12 H15.8")
    out += P("M5 8.5 Q5 5 8.5 5 H15.8")
    for c in ((5, 3), (5, 21), (18, 12), (18, 5)):
        out += circle(*c, 2.2)
    return out


@icon("branch")
def _():
    out = circle(7, 4.8, 2.3) + circle(7, 19.2, 2.3) + circle(17, 7, 2.3)
    out += line((7, 7.1), (7, 16.9))
    out += P("M17 9.3 Q17 14 7 15")
    return out


@icon("version")
def _():
    out = P("M3.5 4.8 Q3.5 3.5 4.8 3.5 H11.2 L20.5 12.8 L12.8 20.5 L3.5 11.2 Z")
    return out + circle(8, 8, 1.6)


@icon("merge")
def _():
    out = P("M6 3 V6 Q6 11 12 13.5") + P("M18 3 V6 Q18 11 12 13.5") + line((12, 13.5), (12, 21))
    out += head((12, 21), (0, 1), 3)
    return out


@icon("undo")
def _():
    return P("M8.5 4.5 L4 9 L8.5 13.5") + P("M4 9 H14.5 A5.5 5.5 0 0 1 14.5 20 H9.5")


@icon("redo")
def _():
    return P("M15.5 4.5 L20 9 L15.5 13.5") + P("M20 9 H9.5 A5.5 5.5 0 0 0 9.5 20 H14.5")


@icon("search")
def _():
    return circle(10.5, 10.5, 7) + line((15.6, 15.6), (21, 21))


@icon("settings")
def _():
    ps = []
    n = 8
    for i in range(n):
        a = 360 * i / n
        for da, r in ((-14, 9.4), (14, 9.4), (24, 6.8), (45 - 24, 6.8)):
            pass
    ps = []
    for i in range(n):
        a0 = 360 * i / n
        ps += [polar((12, 12), 6.9, a0 - 22.5 + 4), polar((12, 12), 9.5, a0 - 11), polar((12, 12), 9.5, a0 + 11),
               polar((12, 12), 6.9, a0 + 22.5 - 4)]
    return poly(*ps) + circle(12, 12, 3)


def tray():
    return P("M3.5 14 V20.5 H20.5 V14")


@icon("export")
def _():
    return tray() + arrow((12, 16), (12, 2.5), s=4)


@icon("export-step")
def _():
    s, _ = cube(9.5, 14, 7, fill=("top", "left", "right"))
    return s + arrow((14.5, 9.5), (21, 3), s=3.2) + line((21, 3), (21, 3))


@icon("export-stl")
def _():
    A, Bp, Cc = (3, 21), (16, 21), (9.5, 8.5)
    ab, bc, ca = lerp(A, Bp, 0.5), lerp(Bp, Cc, 0.5), lerp(Cc, A, 0.5)
    out = fpoly(ab, bc, ca) + poly(A, Bp, Cc) + poly(ab, bc, ca)
    return out + arrow((14.5, 9.5), (21, 3), s=3.2)


def page_d():
    return "M5 2.5 H14 L19 7.5 V20 Q19 21.5 17.5 21.5 H6.5 Q5 21.5 5 20 V4 Q5 2.5 6.5 2.5 Z"


def page():
    return P("M6.5 2.5 H14 L19 7.5 V20 Q19 21.5 17.5 21.5 H6.5 Q5 21.5 5 20 V4 Q5 2.5 6.5 2.5 Z") + P("M14 2.5 V7.5 H19")


@icon("new-document")
def _():
    return page() + line((9, 14.5), (15, 14.5)) + line((12, 11.5), (12, 17.5))


@icon("open-document")
def _():
    out = P("M3 18.5 V5.5 Q3 4.5 4 4.5 H8.5 L10.5 6.5 H17.5 Q18.5 6.5 18.5 7.5 V10")
    fl = "M3 18.5 L6.2 11 Q6.6 10 7.6 10 H21 L17.8 18.5 Z"
    return out + fpath(fl) + P(fl)


@icon("save")
def _():
    out = P("M5 3.5 H16.5 L20.5 7.5 V19 Q20.5 20.5 19 20.5 H5 Q3.5 20.5 3.5 19 V5 Q3.5 3.5 5 3.5 Z")
    out += P("M7.5 3.5 V8 H15 V3.5") + rect(7, 13, 10, 7.5, 0.6)
    return out


@icon("document")
def _():
    return page() + line((8.5, 11.5), (15.5, 11.5)) + line((8.5, 14.5), (15.5, 14.5)) + line((8.5, 17.5), (13, 17.5))


@icon("folder")
def _():
    return P(folder_d())


@icon("chevron-down")
def _():
    return line((6, 9), (12, 15), (18, 9))


@icon("chevron-right")
def _():
    return line((9, 6), (15, 12), (9, 18))


@icon("close")
def _():
    return line((6, 6), (18, 18)) + line((18, 6), (6, 18))


@icon("check")
def _():
    return line((4.5, 12.5), (9.5, 17.5), (19.5, 6.5))


@icon("check-repeat")
def _():
    out = line((2.5, 11.5), (6.5, 15.5), (13.5, 7.5))
    out += arcarrow(16.5, 15.5, 4.6, 4.6, 200, 470 - 360 + 360 - 30, s=2.2) if False else ""
    out += arcarrow(16.5, 15.5, 4.6, 4.6, -100, 170, s=2.2)
    return out


@icon("flip")
def _():
    out = ddline((2, 12), (22, 12))
    out += arrow((8, 12), (8, 3), s=2.6) + arrow((16, 12), (16, 21), s=2.6)
    return out


@icon("flip-circular")
def _():
    out = arcarrow(12, 12, 8, 8, 200, 330, s=2.4) + arcarrow(12, 12, 8, 8, 160, 30, s=2.4)
    return out + dot(12, 12, 1.5)


@icon("rollback")
def _():
    out = line((4, 4.5), (20, 4.5)) + line((4, 8.5), (16, 8.5))
    out += rect(2.5, 11.5, 19, 2.5, 1.2, fill="black", stroke="none")
    out += dline((4, 17.5), (18, 17.5)) + dline((4, 21), (14, 21))
    return out


@icon("suppress")
def _():
    s, _ = cube(12, 12, 8.5)
    return s + line((3, 21), (21, 3))


@icon("unsuppress")
def _():
    s, _ = cube(13, 13.5, 7.5, fill=("top", "left", "right"))
    return s + arcarrow(6.5, 7.5, 4.5, 4.5, 80, 340, s=2.2)


@icon("edit")
def _():
    return pencil((3.5, 20.5), angle=-45, L=20, w=5.4)


@icon("rename")
def _():
    out = rect(2.5, 7, 19, 10, 2)
    out += line((6, 12), (11.5, 12))
    out += line((15.5, 4.5), (15.5, 19.5)) + line((13.5, 4.5), (17.5, 4.5)) + line((13.5, 19.5), (17.5, 19.5))
    return out


@icon("delete")
def _():
    out = line((3.5, 6.5), (20.5, 6.5)) + P("M9 6.5 V4.5 Q9 3.5 10 3.5 H14 Q15 3.5 15 4.5 V6.5")
    out += P("M5.5 6.5 L6.7 19.6 Q6.8 20.5 7.7 20.5 H16.3 Q17.2 20.5 17.3 19.6 L18.5 6.5")
    out += line((10, 10), (10, 17)) + line((14, 10), (14, 17))
    return out


@icon("copy")
def _():
    out = rect(8.5, 8.5, 12.5, 12.5, 2)
    out += P("M15.5 8.5 V5 Q15.5 3 13.5 3 H5 Q3 3 3 5 V13.5 Q3 15.5 5 15.5 H8.5")
    return out


@icon("paste")
def _():
    out = P("M8.5 4.5 H6.5 Q4.5 4.5 4.5 6.5 V19.5 Q4.5 21.5 6.5 21.5 H17.5 Q19.5 21.5 19.5 19.5 V6.5 Q19.5 4.5 17.5 4.5 H15.5")
    out += rect(8.5, 2.5, 7, 4, 1) + frect(8.5, 2.5, 7, 4, 1)
    out += line((8.5, 11.5), (15.5, 11.5)) + line((8.5, 15.5), (13.5, 15.5))
    return out


@icon("error")
def _():
    r = 9.5
    ps = [polar((12, 12), r, 22.5 + 45 * i) for i in range(8)]
    return fpoly(*ps) + poly(*ps) + line((9, 9), (15, 15)) + line((15, 9), (9, 15))


@icon("warning")
def _():
    out = P("M10.3 4 Q12 1.2 13.7 4 L21.3 17.6 Q22.8 20.5 19.5 20.5 H4.5 Q1.2 20.5 2.7 17.6 Z")
    return out + line((12, 9), (12, 14)) + dot(12, 17.2, 1.3)


@icon("info")
def _():
    return circle(12, 12, 9.5) + dot(12, 7.6, 1.4) + line((12, 11), (12, 17))


@icon("under-defined")
def _():
    out = line((3, 20.5), (10, 20.5), (13.5, 10.5)) + dot(3, 20.5, 1.4) + dot(10, 20.5, 1.4)
    out += circle(13.5, 10.5, 2.2)
    out += arrow((16.5, 10.5), (21.5, 10.5), s=2) + arrow((13.5, 7.5), (13.5, 2.5), s=2)
    return out


@icon("plus")
def _():
    return line((12, 4.5), (12, 19.5)) + line((4.5, 12), (19.5, 12))


@icon("minus")
def _():
    return line((4.5, 12), (19.5, 12))


@icon("more")
def _():
    return dot(5.5, 12, 1.8) + dot(12, 12, 1.8) + dot(18.5, 12, 1.8)


@icon("select-box")
def _():
    out = rect(2.5, 2.5, 15, 12, 1, stroke_dasharray=DASH)
    cur = [(11.5, 9.5), (11.5, 21.5), (14.5, 18.5), (16.8, 22.8), (18.6, 21.9), (16.4, 17.8), (20.6, 17.4)]
    return out + fpoly(*cur, op="0.6") + poly(*cur)


@icon("filter")
def _():
    return P("M3.5 4.5 H20.5 L14 12.5 V19 L10 21 V12.5 Z")


@icon("eye")
def _():
    return eye_glyph(12, 12, 10, 4.6, 3.2)


@icon("eye-off")
def _():
    return eye_glyph(12, 12, 10, 4.6, 3.2) + line((4, 20), (20, 4))


@icon("lock")
def _():
    out = rect(4.5, 10.5, 15, 10.5, 2) + P("M8 10.5 V7.5 A4 4 0 0 1 16 7.5 V10.5")
    return out + line((12, 14.5), (12, 17))


@icon("unlock")
def _():
    out = rect(4.5, 10.5, 15, 10.5, 2) + P("M8 10.5 V7 A4 4 0 0 1 15.8 5.8")
    return out + line((12, 14.5), (12, 17))


@icon("link")
def _():
    g = rect(2.5, 8.5, 11, 7, 3.5) + rect(10.5, 8.5, 11, 7, 3.5)
    return f'<g transform="rotate(-45 12 12)">{g}</g>'


@icon("unlink")
def _():
    g = P("M9 8.5 H6 A3.5 3.5 0 0 0 6 15.5 H9") + P("M15 8.5 H18 A3.5 3.5 0 0 1 18 15.5 H15")
    g += line((12, 4.5), (12, 6.5)) + line((12, 17.5), (12, 19.5))
    return f'<g transform="rotate(-45 12 12)">{g}</g>' + line((15.5, 3), (16.4, 5)) if False else \
        f'<g transform="rotate(-45 12 12)">{g}</g>'


@icon("home")
def _():
    out = line((3, 11.5), (12, 3.5), (21, 11.5))
    out += P("M5.5 9.5 V19.5 Q5.5 20.5 6.5 20.5 H17.5 Q18.5 20.5 18.5 19.5 V9.5")
    out += P("M10 20.5 V15 H14 V20.5")
    return out
