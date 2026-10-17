from base import *


@icon("sketch")
def _():
    L, B, R, F = (2, 15.5), (11, 11), (20, 15.5), (11, 20)
    out = fpoly(L, B, R, F, op="0.2") + poly(L, B, R, F)
    out += P("M6 16 C7.5 13.5 10 18 12.6 14.4")
    out += pencil((12.6, 14.4), angle=-45, L=12.5, w=4)
    return out


@icon("extrude")
def _():
    o = center_o(8, 8, 7, 12, 12.5)
    b = Box(o, 8, 8, 7)
    p = b.p
    bot = [p(0, 0, 0), p(8, 0, 0), p(8, 8, 0), p(0, 8, 0)]
    out = fpoly(*bot) + poly(*bot)
    out += dline(p(8, 0, 0), p(8, 0, 7)) + dline(p(8, 8, 0), p(8, 8, 7)) + dline(p(0, 8, 0), p(0, 8, 7))
    out += dline(*b.top(), b.top()[0])
    c = p(4, 4, 0)
    out += arrow(c, (c[0], 1.6), s=3)
    return out


@icon("revolve")
def _():
    out = ddline((8, 1.8), (8, 22.2))
    prof = [(10, 6), (16, 6), (16, 9), (13, 9), (13, 15), (10, 15)]
    out += fpoly(*prof) + poly(*prof)
    out += arcarrow(8, 18.5, 6.5, 2.4, -30, 250, s=2.4)
    return out


@icon("sweep")
def _():
    p0, p1, p2, p3 = (6, 19), (6, 9.5), (18, 14.5), (18, 5.5)
    out = P(C(p0, p1, p2, p3), stroke_dasharray=DASH)
    out += fellipse(6, 19, 3.6, 1.8) + ellipse(6, 19, 3.6, 1.8)
    out += ellipse(18, 5.5, 3.6, 1.8)
    out += line((2.4, 19), (2.6, 14)) + line((9.6, 19), (9.4, 15))
    return out


@icon("loft")
def _():
    L, B, R, F = (4, 18), (12, 14), (20, 18), (12, 22)
    out = fpoly(L, B, R, F) + poly(L, B, R, F)
    out += ellipse(12, 5.5, 5, 2.4)
    out += line(L, (7, 5.5)) + line(R, (17, 5.5)) + dline(F, (12, 7.9))
    return out


@icon("thicken")
def _():
    d, s = sheet(12, 9, amp=1.4, amp2=1.0)
    out = fpath(d) + P(d)
    dv = (0, 5)
    e1 = shift(s["e1"], dv); e2 = shift(s["e2"], dv)
    out += P(curve_d(e1) + " " + C(*e2, move=False))
    out += line(s["L"], add(s["L"], dv)) + line(s["F"], add(s["F"], dv)) + line(s["R"], add(s["R"], dv))
    return out


@icon("enclose")
def _():
    o = center_o(10, 10, 10)
    b = Box(o, 10, 10, 10)
    out = ""
    for face in ("top", "left", "right"):
        ps = getattr(b, face)()
        c = (sum(q[0] for q in ps) / 4, sum(q[1] for q in ps) / 4)
        ps = [lerp(c, q, 0.8) for q in ps]
        out += fpoly(*ps) + poly(*ps)
    return out


def _fillet_prism(o, a=9, r=3.6, fill=True):
    prof = [(0, 0), (a, 0)] + arcpts((a - r, a - r), r, 0, 90, 8) + [(0, a)]
    return prism(o, prof, a, fill_faces=range(2, 10) if fill else (), extra_y_edges=(2, 10))


@icon("fillet")
def _():
    return _fillet_prism(center_o(10, 10, 10), 10, 4)


@icon("chamfer")
def _():
    a, c = 10, 4.5
    prof = [(0, 0), (a, 0), (a, a - c), (a - c, a), (0, a)]
    return prism(center_o(a, a, a), prof, a, fill_faces=(2,))


@icon("face-blend")
def _():
    o = center_o(11, 10, 9, 12, 12)
    p = lambda x, y, z: iso(o, x, y, z)
    r = 4.5
    def arcp(y):
        return [p(r - r * math.sin(math.radians(t)), y, r - r * math.cos(math.radians(t))) for t in range(0, 91, 10)]
    a0, a1 = arcp(0), arcp(10)
    out = fpoly(*a1, *reversed(a0))
    out += line(p(11, 0, 0), *a0[:1]) if False else ""
    d = [M([p(11, 0, 0), p(r, 0, 0)] + a0[1:] + [p(0, 0, 9)]),
         M([p(11, 10, 0), p(r, 10, 0)] + a1[1:] + [p(0, 10, 9)]),
         M([p(11, 0, 0), p(11, 10, 0)]), M([p(0, 0, 9), p(0, 10, 9)])]
    out += P(" ".join(d))
    return out


@icon("draft")
def _():
    o = center_o(10, 10, 9, 12, 12.5)
    p = lambda x, y, z: iso(o, x, y, z)
    t = [p(2.5, 2.5, 9), p(7.5, 2.5, 9), p(7.5, 7.5, 9), p(2.5, 7.5, 9)]
    out = fpoly(p(7.5, 2.5, 9), p(10, 0, 0), p(10, 10, 0), p(7.5, 7.5, 9))
    out += poly(*t)
    out += line(p(7.5, 2.5, 9), p(10, 0, 0), p(10, 10, 0), p(0, 10, 0), p(2.5, 7.5, 9))
    out += line(p(7.5, 7.5, 9), p(10, 10, 0))
    out += dline(p(10, 10, 0), p(10, 10, 11))
    return out


@icon("rib")
def _():
    o = center_o(11, 10, 10, 12, 12)
    p = lambda x, y, z: iso(o, x, y, z)
    tri = [p(0, 5, 0), p(0, 5, 8), p(7.5, 5, 0)]
    out = fpoly(*tri) + poly(*tri)
    out += poly(p(0, 0, 0), p(11, 0, 0), p(11, 10, 0), p(0, 10, 0))
    out += line(p(0, 0, 0), p(0, 0, 10), p(0, 10, 10), p(0, 10, 0))
    return out


@icon("shell")
def _():
    o = center_o(10, 10, 7, 12, 12)
    b = Box(o, 10, 10, 7)
    p = b.p
    w = 1.6
    op = [p(w, w, 7), p(10 - w, w, 7), p(10 - w, 10 - w, 7), p(w, 10 - w, 7)]
    clip = f'<defs><clipPath id="c"><path d="{M(op, True)}"/></clipPath></defs>'
    fl = 1.5
    inner = (fpoly(p(w, w, 7), p(w, 10 - w, 7), p(w, 10 - w, fl), p(w, w, fl)) +
             fpoly(p(w, w, 7), p(10 - w, w, 7), p(10 - w, w, fl), p(w, w, fl)) +
             line(p(w, w, 7), p(w, w, fl)) + line(p(w, 10 - w, fl), p(w, w, fl), p(10 - w, w, fl)))
    out = clip + f'<g clip-path="url(#c)">{inner}</g>'
    out += b.edges() + poly(*op)
    return out


@icon("hole")
def _():
    o = center_o(13, 13, 3.5, 12, 13)
    b = Box(o, 13, 13, 3.5)
    c = iso(o, 6.5, 6.5, 3.5)
    rx, ry = 3.4 * 1.2247, 3.4 * 0.7071
    dep = 3.2
    e = f"M{f(c[0]-rx)} {f(c[1])} a{f(rx)} {f(ry)} 0 1 0 {f(2*rx)} 0 a{f(rx)} {f(ry)} 0 1 0 {f(-2*rx)} 0 Z"
    e2 = f"M{f(c[0]-rx)} {f(c[1]+dep)} a{f(rx)} {f(ry)} 0 1 0 {f(2*rx)} 0 a{f(rx)} {f(ry)} 0 1 0 {f(-2*rx)} 0 Z"
    out = f'<defs><clipPath id="h"><path d="{e}"/></clipPath></defs>'
    out += f'<g clip-path="url(#h)">' + P(e + " " + e2, fill="black", fill_opacity=HLOP, stroke="none", fill_rule="evenodd") + P(e2) + "</g>"
    out += b.edges() + P(e)
    out += ddline((c[0], 1.5), (c[0], c[1] - 3.2))
    return out


@icon("external-thread")
def _():
    cx, top, bot, rx, ry = 12, 5, 19, 6, 2.2
    out = ellipse(cx, top, rx, ry)
    out += line((cx - rx, top), (cx - rx, bot)) + line((cx + rx, top), (cx + rx, bot))
    out += arc(cx, bot, rx, ry, 0, 180)
    for y in (8.6, 12.2, 15.8):
        out += P(f"M{f(cx-rx)} {f(y-1)} Q{f(cx)} {f(y+3.6)} {f(cx+rx)} {f(y+1)}")
    return out


@icon("boolean")
def _():
    sq = "M3.5 3.5 H14.5 V14.5 H3.5 Z"
    cd = "M20.5 15 A5.5 5.5 0 1 1 9.5 15 A5.5 5.5 0 1 1 20.5 15 Z"
    out = f'<defs><clipPath id="b"><path d="{sq}"/></clipPath></defs>'
    out += f'<g clip-path="url(#b)">' + fpath(cd, op="0.6") + "</g>"
    out += P(sq) + P(cd)
    return out


@icon("split")
def _():
    o = center_o(10, 10, 12, 12, 12)
    lo = Box(o, 10, 10, 4.5)
    hi = Box(o, 10, 10, 4.5, z0=7.5)
    return fpoly(*lo.top()) + lo.edges() + hi.edges()


@icon("delete-part")
def _():
    s, _b = cube(9.5, 9.5, 7.2)
    return s + badge_x(18.5, 18.5)


@icon("delete-face")
def _():
    s, _b = cube(9.5, 9.5, 7.2, fill=("top",))
    return s + badge_x(18.5, 18.5)


@icon("transform")
def _():
    g, _ = cube(7, 16.5, 5.2, stroke_dasharray=DASH)
    s, _ = cube(17, 7.5, 5.2, fill=("top", "left", "right"))
    out = g + s
    out += P("M11.5 19 Q17 19 17 15") + head((17, 15), (0, -1), 2.4)
    return out


@icon("linear-pattern")
def _():
    out = ""
    for i, x in enumerate((4.9, 12, 19.1)):
        s, _ = cube(x, 9, 3.7, fill=("top", "left", "right") if i == 0 else ())
        out += s
    out += arrow((3.5, 18.5), (20.5, 18.5), s=2.4)
    return out


@icon("circular-pattern")
def _():
    out = ""
    R = 7.6
    for i, a in enumerate((-90, 30, 150)):
        c = polar((12, 12.5), R, a)
        s, _ = cube(c[0], c[1], 3, fill=("top", "left", "right") if i == 0 else ())
        out += s
    out += arcarrow(12, 12.5, R, R, -64, 4, s=2.2)
    out += arcarrow(12, 12.5, R, R, 56, 124, s=2.2)
    out += dot(12, 12.5, 1.3)
    return out


@icon("curve-pattern")
def _():
    p0, p1, p2 = (2.5, 21), (4, 9), (21.5, 9)
    def q(t):
        u = 1 - t
        return (u * u * p0[0] + 2 * u * t * p1[0] + t * t * p2[0], u * u * p0[1] + 2 * u * t * p1[1] + t * t * p2[1])
    out = P(Q(p0, p1, p2), stroke_dasharray=DASH)
    for i, t in enumerate((0.12, 0.5, 0.9)):
        c = q(t)
        s, _ = cube(c[0], c[1] - 4.2, 3.1, fill=("top", "left", "right") if i == 0 else ())
        out += s
    return out


def _wedge(cx, cy, fill):
    o = center_o(6, 5, 7, cx, cy)
    prof = [(0, 0), (6, 0), (0, 7)]
    return prism(o, prof, 5, fill_cap=fill, fill_faces=(1,) if fill else ())


@icon("mirror")
def _():
    w = _wedge(5.6, 12, True)
    w2 = _wedge(5.6, 12, False)
    return w + f'<g transform="matrix(-1 0 0 1 24 0)">{w2}</g>' + ddline((12, 2), (12, 22))


@icon("move-face")
def _():
    s, b = cube(9.5, 12, 8.4, fill=("right",))
    c = b.center("right")
    return s + arrow(c, add(c, (0.866 * 8.5, 0.5 * 8.5)), s=2.6)


@icon("replace-face")
def _():
    o = center_o(10, 10, 8, 12, 13.5)
    p = lambda x, y, z: iso(o, x, y, z)
    A, Bp, Cp, D = p(0, 0, 8), p(10, 0, 8), p(10, 10, 8), p(0, 10, 8)
    bulge = (0, -4.5)
    def qc(a, b):
        return add(lerp(a, b, 0.5), bulge)
    top = (f"M{pt(D)} Q{pt(qc(D, Cp))} {pt(Cp)} Q{pt(qc(Cp, Bp))} {pt(Bp)} "
           f"Q{pt(qc(Bp, A))} {pt(A)} Q{pt(qc(A, D))} {pt(D)} Z")
    out = fpath(top) + P(top)
    out += line(D, p(0, 10, 0), p(10, 10, 0), p(10, 0, 0), Bp) + line(Cp, p(10, 10, 0))
    return out


@icon("offset-surface")
def _():
    d1, s1 = sheet(12, 15.5, amp=1.4, amp2=1.0)
    d2, s2 = sheet(12, 7.5, amp=1.4, amp2=1.0)
    out = fpath(d1) + P(d1) + P(d2)
    out += arrow((4, 14.5), (4, 9.6), s=2) + arrow((20, 14.5), (20, 9.6), s=2)
    return out


@icon("fill")
def _():
    A, Bp, Cc = (3.5, 17), (12, 3.5), (20.5, 16)
    d = (f"M{pt(A)} C6 11 7 6 {pt(Bp)} C16 6 19 10 {pt(Cc)} C15 21 9 14 {pt(A)} Z")
    return fpath(d) + P(d) + dot(*A) + dot(*Bp) + dot(*Cc)


@icon("move-boundary")
def _():
    d, s = sheet(10, 11, amp=1.4, amp2=1.0)
    out = fpath(d) + P(d)
    v = (4.5, 2.6)
    e2 = s["e2"]
    out += P(C(*shift(e2, v)), stroke_dasharray=DASH)
    out += dline(s["F"], add(s["F"], v)) + dline(s["R"], add(s["R"], v))
    m = bez(*e2, 0.5)
    out += arrow(m, add(m, (3.6, 2.1)), s=2)
    return out


@icon("boundary-surface")
def _():
    d, s = sheet(12, 12, w=9, h=5.2, amp=1.6, amp2=1.2)
    out = P(d)
    for t in (0.5,):
        v = sub(bez(*s["e2"], t), s["F"])
        out += P(C(*shift(s["e1"], v)), stroke_dasharray=DASH)
        v2 = sub(bez(*s["e1"], t), s["L"])
        out += P(C(*shift(s["e2"], v2)), stroke_dasharray=DASH)
    for k in ("L", "F", "R", "B"):
        out += dot(*s[k], 1.7)
    return out


@icon("ruled-surface")
def _():
    a = ((3, 8), (8, 2), (15, 11), (21, 5.5))
    bb = ((3, 18.5), (9, 16.5), (15, 20.5), (21, 17))
    d = C(*a) + f" L{pt(bb[3])} C{pt(bb[2])} {pt(bb[1])} {pt(bb[0])} Z"
    out = fpath(d, op="0.2") + P(C(*a)) + P(C(*bb))
    for t in (0, 0.25, 0.5, 0.75, 1):
        out += line(bez(*a, t), bez(*bb, t))
    return out


@icon("helix")
def _():
    out = ddline((12, 1.5), (12, 22.5))
    ps = []
    turns, h, r = 2.25, 13, 5.2
    for i in range(0, 241):
        t = i / 240
        a = 2 * math.pi * turns * t + math.pi * 0.75
        x, y, z = r * math.cos(a), r * math.sin(a), h * t
        ps.append(iso((12, 18.5), x, y, z))
    out += P(smooth(ps[::6] + [ps[-1]]))
    return out


@icon("projected-curve")
def _():
    L, B, R, F = (2, 17), (12, 12), (22, 17), (12, 22)
    out = fpoly(L, B, R, F, op="0.2") + poly(L, B, R, F)
    a = ((5, 6), (9, 1.5), (14, 9), (19, 4.5))
    out += P(C(*a))
    bq = shift(a, (0, 11))
    out += P(C(*bq), stroke_width="2.2")
    out += dline(a[0], bq[0]) + dline(a[3], bq[3])
    return out


@icon("bridging-curve")
def _():
    out = line((2, 17.5), (8, 17.5)) + line((16, 6.5), (22, 6.5))
    out += P("M8 17.5 C13.5 17.5 10.5 6.5 16 6.5", stroke_dasharray=DASH)
    out += dot(8, 17.5, 1.8) + dot(16, 6.5, 1.8)
    return out


@icon("composite-curve")
def _():
    out = P("M2.5 18 L8 18 A5 5 0 0 0 13 13 L13 10 Q13 6 17 6 L21.5 6")
    for p_ in ((8, 18), (13, 13), (13, 10), (17, 6)):
        out += dot(*p_, 1.6)
    return out


@icon("intersection-curve")
def _():
    o = center_o(12, 12, 10, 12, 12)
    p = lambda x, y, z: iso(o, x, y, z)
    hz = [p(0, 0, 5), p(12, 0, 5), p(12, 12, 5), p(0, 12, 5)]
    out = fpoly(*hz, op="0.2") + poly(*hz)
    out += line(p(0, 6, 5), p(0, 6, 10), p(12, 6, 10), p(12, 6, 5))
    out += dline(p(0, 6, 5), p(0, 6, 0), p(12, 6, 0), p(12, 6, 5))
    out += line(p(0, 6, 5), p(12, 6, 5), stroke_width="2.6")
    return out


@icon("fit-spline")
def _():
    a = ((3, 18), (6, 3), (14, 22), (21, 6))
    out = P(C(*a))
    for t in (0, 0.36, 0.66, 1):
        c = bez(*a, t)
        out += line(add(c, (-1.8, -1.8)), add(c, (1.8, 1.8)), stroke_width="1.3")
        out += line(add(c, (-1.8, 1.8)), add(c, (1.8, -1.8)), stroke_width="1.3")
    return out


@icon("offset-curve")
def _():
    out = arc(12, 22, 7, 7, 200, 340, stroke_dasharray=DASH)
    out += arc(12, 22, 13, 13, 205, 335)
    out += arrow((12, 15), (12, 9.6), s=2.2)
    return out


@icon("plane")
def _():
    o = center_o(13, 11, 0)
    p = lambda x, y: iso(o, x, y, 0)
    q = [p(0, 0), p(13, 0), p(13, 11), p(0, 11)]
    return fpoly(*q) + poly(*q)


@icon("mate-connector")
def _():
    out = fellipse(12, 16, 7.5, 3.6) + ellipse(12, 16, 7.5, 3.6) + dot(12, 16, 1.6)
    out += arrow((12, 16), (12, 2.5), s=2.8)
    out += line((12, 16), (17.2, 19)) if False else ""
    return out


@icon("point")
def _():
    out = dot(12, 12, 2.3)
    for d in ((0, -1), (C30, 0.5), (-C30, 0.5)):
        out += line(add((12, 12), mul(d, 4.6)), add((12, 12), mul(d, 9.5)))
    return out


def _x_glyph(cx, cy, s):
    return line((cx - s, cy - s), (cx + s, cy + s)) + line((cx + s, cy - s), (cx - s, cy + s))


@icon("variable")
def _():
    out = P("M8 3.5 Q3.5 12 8 20.5") + P("M16 3.5 Q20.5 12 16 20.5")
    out += _x_glyph(12, 12, 2.8)
    return out


@icon("variable-studio")
def _():
    out = rect(3, 3, 18, 18, 2.5) + line((3, 8), (21, 8)) + frect(3, 3, 18, 5, 2.5, op="0.35")
    out += P("M9 10.5 Q6.5 14.5 9 18.5") + P("M15 10.5 Q17.5 14.5 15 18.5")
    out += _x_glyph(12, 14.5, 1.9)
    return out


@icon("import")
def _():
    out = P("M3.5 14 V20.5 H20.5 V14")
    out += arrow((12, 2.5), (12, 15.5), s=4)
    return out


@icon("wrap")
def _():
    cx, top, bot, rx, ry = 12, 4.5, 19.5, 7, 2.6
    out = ellipse(cx, top, rx, ry)
    out += line((cx - rx, top), (cx - rx, bot)) + line((cx + rx, top), (cx + rx, bot))
    out += arc(cx, bot, rx, ry, 0, 180)
    a0, a1 = 25, 145
    up = arcd(cx, 9, rx, ry, a0, a1)
    lo = arcd(cx, 15.5, rx, ry, a1, a0, move=False)
    p1 = polar((cx, 15.5), rx, a1, ry)
    d = up + f" L{pt(p1)} " + lo + " Z"
    out += fpath(d) + P(d)
    return out


def _L_profile(base=11, h=10, r=3):
    pts = [(base, 0)] + arcpts((r, r), r, 270, 180, 6) + [(0, h)]
    return pts


@icon("sheet-metal")
def _():
    o = center_o(11, 9, 10, 12, 12)
    pts = _L_profile()
    n = len(pts)
    out = strip(o, pts, 9, fill_segs=range(0, n - 1), op="0.2")
    out += strip(o, pts, 9, crease=(1, n - 2)) if False else ""
    # bend tangent lines
    out += line(iso(o, 3, 9, 0), iso(o, 3, 0, 0)) + line(iso(o, 0, 9, 3), iso(o, 0, 0, 3))
    return out


@icon("flange")
def _():
    o = center_o(11, 9, 9, 12, 13)
    pts = _L_profile(11, 7.5, 2.6)
    n = len(pts)
    out = strip(o, pts, 9, fill_segs=(n - 2,))
    out += line(iso(o, 2.6, 9, 0), iso(o, 2.6, 0, 0))
    top = iso(o, 0, 4.5, 7.5)
    out += arrow(add(top, (0, -1)), add(top, (0, -7.5)), s=2.4)
    return out


@icon("hem")
def _():
    o = center_o(12, 9, 5, 12, 12.5)
    pts = [(12, 0), (2.5, 0)] + arcpts((2.5, 2.2), 2.2, 270, 90, 8)[1:] + [(8, 4.4)]
    n = len(pts)
    segs = list(range(1, n - 1))
    out = strip(o, pts, 9, fill_segs=segs)
    return out


@icon("tab")
def _():
    o = center_o(15, 10, 1.6, 11.5, 12.5)
    poly_xy = [(0, 0), (10, 0), (10, 3), (15, 3), (15, 7), (10, 7), (10, 10), (0, 10)]
    out = slab(o, poly_xy, 1.6)
    tab = [iso(o, 10, 3, 1.6), iso(o, 15, 3, 1.6), iso(o, 15, 7, 1.6), iso(o, 10, 7, 1.6)]
    out = fpoly(*tab) + out
    return out


@icon("bend-relief")
def _():
    o = center_o(13, 13, 1.5, 12, 12)
    nw, nd = 2.8, 3.4
    x0 = 3.6
    poly_xy = [(0, 0), (x0, 0), (x0, nd), (x0 + nw, nd), (x0 + nw, 0), (13, 0), (13, 13),
               (x0 + nw, 13), (x0 + nw, 13 - nd), (x0, 13 - nd), (x0, 13), (0, 13)]
    out = slab(o, poly_xy, 1.5)
    for y0, y1 in ((0, nd), (13 - nd, 13)):
        out += fpoly(iso(o, x0, y0, 0), iso(o, x0 + nw, y0, 0), iso(o, x0 + nw, y1, 0), iso(o, x0, y1, 0), op="0.5")
    out += dline(iso(o, x0 + nw / 2, nd, 1.5), iso(o, x0 + nw / 2, 13 - nd, 1.5))
    return out


@icon("corner-break")
def _():
    o = center_o(14, 14, 1.5, 12, 12)
    r = 5.5
    poly_xy = [(0, 0), (14, 0)] + [(14 - r + r * math.cos(math.radians(a)), 14 - r + r * math.sin(math.radians(a)))
                                  for a in range(0, 91, 10)] + [(0, 14)]
    out = slab(o, poly_xy, 1.5)
    cut = [iso(o, 14, 14 - r, 1.5), iso(o, 14, 14, 1.5), iso(o, 14 - r, 14, 1.5)]
    arcp = [iso(o, 14 - r + r * math.cos(math.radians(a)), 14 - r + r * math.sin(math.radians(a)), 1.5) for a in range(90, -1, -10)]
    out = fpoly(*cut[1:2], *arcp) + out
    out += dline(*cut)
    return out


@icon("frame")
def _():
    t = 3.4
    o = center_o(13, t, 13, 12, 12)
    prof = [(0, 0), (13, 0), (13, t), (t, t), (t, 13), (0, 13)]
    out = prism(o, prof, t, fill_faces=(1, 4))
    out += line(iso(o, 0, t, 0), iso(o, t, t, t))
    return out


@icon("gusset")
def _():
    t = 3.2
    o = center_o(13, t, 13, 12, 12)
    prof = [(0, 0), (13, 0), (13, t), (t, t), (t, 13), (0, 13)]
    out = prism(o, prof, t)
    tri = [iso(o, t, t, t), iso(o, 10, t / 2, t), iso(o, t, t / 2, 10)]
    tri = [iso(o, t, t / 2, t), iso(o, 10.5, t / 2, t), iso(o, t, t / 2, 10.5)]
    out += fpoly(*tri, op="0.5") + poly(*tri)
    return out


@icon("end-cap")
def _():
    o = center_o(14, 5, 5, 12, 12)
    b = Box(o, 14, 5, 5)
    out = fpoly(*b.right()) + b.edges()
    out += line(iso(o, 12.3, 5, 0), iso(o, 12.3, 5, 5), iso(o, 12.3, 0, 5))
    return out


@icon("frame-trim")
def _():
    o = center_o(14, 5, 5, 12, 12)
    prof = [(0, 0), (14, 0), (9.5, 5), (0, 5)]
    out = prism(o, prof, 5, axis="z", fill_faces=(1,)) if False else ""
    out += slab(o, prof, 5, fill_side=(1,))
    out += dline(iso(o, 9.5, 5, 5), iso(o, 14, 5, 5), iso(o, 14, 0, 5))
    out += dline(iso(o, 14, 5, 5), iso(o, 14, 5, 0), iso(o, 9.5, 5, 0))
    return out


@icon("cut-list")
def _():
    out = rect(3.5, 3, 17, 18, 2)
    for i, y in enumerate((7, 12, 17)):
        out += rect(6.2, y - 1.4, 2.8, 2.8, 0.3) + line((11.5, y), (17.8, y))
    return out


@icon("configuration")
def _():
    out = ""
    for y, k in ((6, 15), (12, 8), (18, 13)):
        out += line((3.5, y), (k - 2.3, y)) + line((k + 2.3, y), (20.5, y))
        out += fcircle(k, y, 2.3) + circle(k, y, 2.3)
    return out


def folder_d():
    return "M2.8 6 Q2.8 4.5 4.3 4.5 H9 L11 6.8 H19.7 Q21.2 6.8 21.2 8.3 V18 Q21.2 19.5 19.7 19.5 H4.3 Q2.8 19.5 2.8 18 Z"


@icon("feature-folder")
def _():
    s, _ = cube(12, 13.4, 3.6, fill=("top",))
    return P(folder_d()) + s


@icon("modify-fillet")
def _():
    o = center_o(10, 10, 10, 12, 12)
    out = _fillet_prism(o, 10, 4.6)
    c = iso(o, 5.4, 10, 5.4)
    e = iso(o, 5.4 + 4.6 * 0.707, 10, 5.4 + 4.6 * 0.707)
    out += arrow(c, e, s=2) + dot(*c, 1.2)
    return out


@icon("body-draft")
def _():
    o = center_o(10, 10, 12, 12, 12)
    p = lambda x, y, z: iso(o, x, y, z)
    a, b = 2.2, 7.8
    out = fpoly(p(b, a, 12), p(10, 0, 6), p(10, 10, 6), p(b, b, 12))
    out += fpoly(p(b, a, 0), p(10, 0, 6), p(10, 10, 6), p(b, b, 0), op="0.18")
    out += poly(p(a, a, 12), p(b, a, 12), p(b, b, 12), p(a, b, 12))
    out += line(p(b, a, 12), p(10, 0, 6), p(b, a, 0), p(b, b, 0), p(a, b, 0), p(0, 10, 6), p(a, b, 12))
    out += line(p(b, b, 12), p(10, 10, 6), p(b, b, 0))
    out += line(p(10, 0, 6), p(10, 10, 6), p(0, 10, 6))
    out += dline(p(0, 10, 6), add(p(0, 10, 6), (-1.5, -0.9))) if False else ""
    return out


@icon("isocline")
def _():
    cx, cy, rx, ry = 12, 18.5, 9, 12
    out = arc(cx, cy, rx, ry, 180, 360)
    out += arc(cx, cy, rx, 3.4, 0, 180)
    out += arc(cx, cy, rx, 3.4, 180, 360, stroke_dasharray=DASH)
    yy = 12.5
    k = math.sqrt(1 - ((cy - yy) / ry) ** 2)
    rr = rx * k
    ry2 = 3.4 * k
    a = (cx - rr, yy); b = (cx + rr, yy)
    cap = (f"M{pt(a)} A{f(rx)} {f(ry)} 0 0 1 {pt(b)} A{f(rr)} {f(ry2)} 0 0 1 {pt(a)} Z")
    out += fpath(cap) + ellipse(cx, yy, rr, ry2)
    return out


@icon("isoparametric")
def _():
    d, s = sheet(12, 12, w=9, h=5.2)
    out = fpath(d, op="0.2") + P(d)
    for t in (0.33, 0.66):
        v = sub(bez(*s["e2"], t), s["F"])
        out += P(C(*shift(s["e1"], v)), stroke_width="1.5" if t < 0.5 else "1.5")
    return out


@icon("routing-curve")
def _():
    d = "M4 20 V12.5 Q4 9.5 6.6 8 L11 5.5 Q13 4.4 15 5.5 L18.6 7.6 Q20.5 8.7 20.5 11 V16"
    out = P(d)
    out += dot(4, 20, 1.8) + dot(20.5, 16, 1.8)
    for c in ((4, 9.5), (13, 4.4), (20.5, 8.5)):
        out += rect(c[0] - 1.3, c[1] - 1.3, 2.6, 2.6, stroke_width="1")
    return out


@icon("edit-curve")
def _():
    a = ((3, 19), (5, 5), (13, 21), (21, 5))
    out = line(*a, stroke_dasharray=DASH, stroke_width="1.2")
    out += P(C(*a))
    for c in a:
        out += rect(c[0] - 1.5, c[1] - 1.5, 3, 3, fill="black", stroke="none")
    return out


@icon("trim-curve")
def _():
    out = P("M3 18 C6 9 10 8 13 10.5")
    out += P("M13 10.5 C16 13 18 12 21.5 5", stroke_dasharray=DASH)
    out += line((10.6, 14.2), (15.4, 6.8))
    out += dot(13, 10.5, 1.5)
    return out


@icon("mutual-trim")
def _():
    o = center_o(12, 12, 10, 12, 12)
    p = lambda x, y, z: iso(o, x, y, z)
    out = fpoly(p(0, 0, 5), p(0, 12, 5), p(6, 12, 5), p(6, 0, 5), op="0.35")
    out += line(p(6, 0, 5), p(0, 0, 5), p(0, 12, 5), p(6, 12, 5))
    out += dline(p(6, 12, 5), p(12, 12, 5), p(12, 0, 5), p(6, 0, 5))
    out += fpoly(p(6, 0, 5), p(6, 0, 10), p(6, 12, 10), p(6, 12, 5), op="0.2")
    out += line(p(6, 0, 5), p(6, 0, 10), p(6, 12, 10), p(6, 12, 5))
    out += dline(p(6, 12, 5), p(6, 12, 0), p(6, 0, 0), p(6, 0, 5))
    out += line(p(6, 0, 5), p(6, 12, 5))
    return out


@icon("constrained-surface")
def _():
    d, s = sheet(12, 12, w=9, h=5.2)
    out = P(d)
    for c in ((8, 11), (12, 14.5), (15.5, 10.5), (11.5, 9), (16, 14)):
        out += dot(*c, 1.4)
    return out


@icon("form")
def _():
    o = center_o(14, 14, 1.5, 12, 13.5)
    poly_xy = [(0, 0), (14, 0), (14, 14), (0, 14)]
    out = slab(o, poly_xy, 1.5)
    c = iso(o, 7, 7, 1.5)
    rx, ry = 4 * 1.2247, 4 * 0.7071
    dome = (f"M{f(c[0]-rx)} {f(c[1])} A{f(rx)} {f(rx*0.95)} 0 0 1 {f(c[0]+rx)} {f(c[1])} "
            f"A{f(rx)} {f(ry)} 0 0 1 {f(c[0]-rx)} {f(c[1])} Z")
    out += fpath(dome) + P(dome)
    return out


@icon("joint")
def _():
    o = center_o(11, 11, 10, 12, 12)
    p = lambda x, y, z: iso(o, x, y, z)
    g = 1.4
    r = [p(11, 0, 0), p(11, 11 - g, 0), p(11, 11 - g, 10), p(11, 0, 10)]
    l = [p(0, 11, 0), p(11 - g, 11, 0), p(11 - g, 11, 10), p(0, 11, 10)]
    out = fpoly(*r) + poly(*r) + fpoly(*l, op="0.2") + poly(*l)
    out += line(p(11, 0, 10), p(0, 0, 10), p(0, 11, 10)) if False else ""
    return out


@icon("jog")
def _():
    o = center_o(14, 9, 7, 12, 12)
    pts = [(0, 7), (4.5, 7), (8.5, 0.5), (14, 0.5)]
    pts = [(0, 7), (4.2, 7)] + [(4.2 + 1.8 * math.sin(math.radians(a)), 7 - 1.8 + 1.8 * math.cos(math.radians(a))) for a in (20, 45)] + \
          [(8, 1.6), (9.6, 0.5), (14, 0.5)]
    pts = [(0, 7.5), (4.5, 7.5), (9, 0.5), (14, 0.5)]
    out = strip(o, pts, 9, fill_segs=(1,), crease=(1, 2))
    return out
