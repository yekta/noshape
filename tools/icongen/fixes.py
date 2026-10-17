"""Revisions after visual review (later registrations override earlier ones)."""
from base import *
from constraints import ground
import part as _part


# ---------------- part studio ----------------
@icon("extrude")
def _():
    o = center_o(9, 9, 6, 12, 14)
    b = Box(o, 9, 9, 6)
    c = b.center("top")
    return fpoly(*b.top()) + b.edges() + arrow(c, (c[0], 1.6), s=3)


def offset_curve(c, dist, n=24):
    ps = []
    for i in range(n + 1):
        t = i / n
        p = bez(*c, t)
        d = norm(bezt(*c, t))
        ps.append(add(p, mul((-d[1], d[0]), dist)))
    return ps


@icon("sweep")
def _():
    c = ((6, 18.5), (6, 9.5), (18, 14.5), (18, 5.5))
    w = 3
    a = offset_curve(c, w)
    b = offset_curve(c, -w)
    out = P(smooth(a[::4] + [a[-1]])) + P(smooth(b[::4] + [b[-1]]))
    out += fellipse(6, 18.5, w, 1.3) + ellipse(6, 18.5, w, 1.3)
    out += ellipse(18, 5.5, w, 1.3)
    out += P(C(*c), stroke_dasharray=DASH, stroke_width="1.2")
    return out


@icon("linear-pattern")
def _():
    out = ""
    for i, (x, y) in enumerate(((5, 6), (12, 10), (19, 14))):
        s, _ = cube(x, y, 4, fill=("top", "left", "right") if i == 0 else ())
        out += s
    out += arrow((3, 14.5), (12, 19.7), s=2.4)
    return out


@icon("circular-pattern")
def _():
    out = ""
    R = 7.6
    for i, a in enumerate((-90, 30, 150)):
        c = polar((12, 12.6), R, a)
        s, _ = cube(c[0], c[1], 3.5, fill=("top", "left", "right") if i == 0 else ())
        out += s
    out += arcarrow(12, 12.6, R, R, -62, 2, s=2.2)
    out += arcarrow(12, 12.6, R, R, 58, 122, s=2.2)
    out += dot(12, 12.6, 1.4)
    return out


@icon("curve-pattern")
def _():
    p0, p1, p2 = (4, 21.5), (5, 9), (21.5, 9)
    def q(t):
        u = 1 - t
        return (u * u * p0[0] + 2 * u * t * p1[0] + t * t * p2[0], u * u * p0[1] + 2 * u * t * p1[1] + t * t * p2[1])
    out = P(Q(p0, p1, p2), stroke_dasharray=DASH)
    for i, t in enumerate((0.1, 0.5, 0.92)):
        c = q(t)
        s, _ = cube(c[0], c[1] - 4.4, 3.3, fill=("top", "left", "right") if i == 0 else ())
        out += s
    return out


@icon("corner-break")
def _():
    W, r, t = 11, 5.5, 1.6
    o = center_o(W, W, t, 12, 12)
    arcp = [(W - r + r * math.cos(math.radians(a)), r + r * math.sin(math.radians(a))) for a in range(-90, 1, 10)]
    poly_xy = [(0, 0)] + arcp + [(W, W), (0, W)]
    out = fpoly(iso(o, W, 0, t), *[iso(o, x, y, t) for x, y in reversed(arcp)], op="0.6")
    out += slab(o, poly_xy, t)
    out += dline(iso(o, W - r, 0, t), iso(o, W, 0, t), iso(o, W, r, t))
    return out


@icon("form")
def _():
    W, t = 12, 1.5
    o = center_o(W, W, t, 12, 13.5)
    out = slab(o, [(0, 0), (W, 0), (W, W), (0, W)], t)
    c = iso(o, W / 2, W / 2, t)
    rx, ry = 3.6 * 1.2247, 3.6 * 0.7071
    dome = (f"M{f(c[0]-rx)} {f(c[1])} A{f(rx)} {f(rx*0.9)} 0 0 1 {f(c[0]+rx)} {f(c[1])} "
            f"A{f(rx)} {f(ry)} 0 0 1 {f(c[0]-rx)} {f(c[1])} Z")
    return out + fpath(dome) + P(dome)


@icon("bend-relief")
def _():
    W, t = 12, 1.5
    o = center_o(W, W, t, 12, 12)
    nw, nd, x0 = 2.8, 3.4, 3.4
    poly_xy = [(0, 0), (x0, 0), (x0, nd), (x0 + nw, nd), (x0 + nw, 0), (W, 0), (W, W),
               (x0 + nw, W), (x0 + nw, W - nd), (x0, W - nd), (x0, W), (0, W)]
    out = ""
    for y0, y1 in ((0, nd), (W - nd, W)):
        out += fpoly(iso(o, x0, y0, t), iso(o, x0 + nw, y0, t), iso(o, x0 + nw, y1, t), iso(o, x0, y1, t), op="0.5")
    out += slab(o, poly_xy, t)
    out += dline(iso(o, x0 + nw / 2, nd, t), iso(o, x0 + nw / 2, W - nd, t))
    return out


@icon("hem")
def _():
    o = center_o(13, 8, 6, 12, 12.5)
    pts = [(13, 0), (3, 0)] + arcpts((3, 3), 3, 270, 90, 10)[1:] + [(8.5, 6)]
    return strip(o, pts, 8, fill_segs=range(1, len(pts) - 1))


@icon("hole")
def _():
    W, T = 12, 3.5
    o = center_o(W, W, T, 12, 13.5)
    b = Box(o, W, W, T)
    c = iso(o, W / 2, W / 2, T)
    rx, ry = 3.2 * 1.2247, 3.2 * 0.7071
    dep = 3.2
    e = f"M{f(c[0]-rx)} {f(c[1])} a{f(rx)} {f(ry)} 0 1 0 {f(2*rx)} 0 a{f(rx)} {f(ry)} 0 1 0 {f(-2*rx)} 0 Z"
    e2 = f"M{f(c[0]-rx)} {f(c[1]+dep)} a{f(rx)} {f(ry)} 0 1 0 {f(2*rx)} 0 a{f(rx)} {f(ry)} 0 1 0 {f(-2*rx)} 0 Z"
    out = f'<defs><clipPath id="h"><path d="{e}"/></clipPath></defs>'
    out += '<g clip-path="url(#h)">' + P(e + " " + e2, fill="black", fill_opacity="0.5", stroke="none", fill_rule="evenodd") + P(e2) + "</g>"
    out += b.edges() + P(e)
    out += ddline((c[0], 1.8), (c[0], c[1] - 3))
    return out


@icon("flange")
def _():
    o = center_o(11, 9, 8, 12, 13.5)
    pts = _part._L_profile(11, 7.5, 2.6)
    out = strip(o, pts, 9, fill_segs=(len(pts) - 2,))
    out += line(iso(o, 2.6, 9, 0), iso(o, 2.6, 0, 0))
    top = iso(o, 0, 4.5, 7.5)
    return out + arrow(add(top, (0, -1)), add(top, (0, -6.5)), s=2.4)


@icon("intersection-curve")
def _():
    o = center_o(12, 12, 10, 12, 13.5)
    p = lambda x, y, z: iso(o, x, y, z)
    hz = [p(0, 0, 5), p(12, 0, 5), p(12, 12, 5), p(0, 12, 5)]
    vt = [p(0, 6, 5), p(0, 6, 11), p(12, 6, 11), p(12, 6, 5)]
    out = fpoly(*hz, op="0.2") + poly(*hz)
    out += fpoly(*vt, op="0.2") + line(*vt)
    out += line(p(0, 6, 5), p(12, 6, 5), stroke_width="2.6")
    return out


@icon("mutual-trim")
def _():
    o = center_o(13, 10, 13, 12, 12)
    D = 10
    p = lambda s, y, z: iso(o, s, y, z)
    out = strip(o, [(13, 4), (4, 4), (4, 13)], D, fill_segs=(0, 1), crease=(1,), op="0.3")
    out += dline(p(4, D, 4), p(0, D, 4), p(0, 0, 4))
    out += dline(p(4, D, 4), p(4, D, 0), p(4, 0, 0))
    return out


@icon("isoparametric")
def _():
    d, s = sheet(12, 12, w=9.5, h=5.5, amp=1.8, amp2=1.3)
    out = fpath(d, op="0.2") + P(d)
    for t in (0.33, 0.66):
        v = sub(bez(*s["e2"], t), s["F"])
        out += P(C(*shift(s["e1"], v)))
    return out


@icon("offset-curve")
def _():
    out = arc(12, 21, 6.5, 6.5, 200, 340, stroke_dasharray=DASH)
    out += arc(12, 21, 11.5, 11.5, 205, 335)
    out += arrow((12, 14.5), (12, 10.2), s=2.2)
    return out


# ---------------- sketch ----------------
REF = dict(stroke_width="1", stroke_opacity="0.55")


def _ngon(c, r, n=6, phase=-90):
    return [polar(c, r, phase + 360 * i / n) for i in range(n)]


@icon("polygon-inscribed")
def _():
    return circle(12, 12, 9, **REF) + poly(*_ngon((12, 12), 9)) + dot(12, 12, 1.4)


@icon("polygon-circumscribed")
def _():
    return circle(12, 12, 7.4, **REF) + poly(*_ngon((12, 12), 7.4 / C30, phase=0)) + dot(12, 12, 1.4)


@icon("polyline")
def _():
    ps = [(3, 18.5), (6.5, 7), (12, 15.5), (16, 5.5), (21, 13)]
    return line(*ps) + "".join(dot(*p, 1.6) for p in ps)


@icon("spline")
def _():
    ps = [(3, 13), (8, 6), (16, 18), (21, 11)]
    return P(smooth(ps)) + "".join(dot(*p, 1.6) for p in ps)


@icon("use")
def _():
    L, B, R, F = (2, 16.5), (12, 11.5), (22, 16.5), (12, 21.5)
    out = poly(L, B, R, F)
    s, b = cube(12, 6.5, 5)
    fp = [(7.67, 16.5), (12, 14), (16.33, 16.5), (12, 19)]
    return out + s + fpoly(*fp, op="0.6") + poly(*fp)


@icon("intersection")
def _():
    L, B, R, F = (1.5, 14), (12, 8.5), (22.5, 14), (12, 19.5)
    out = fpoly(L, B, R, F, op="0.15") + poly(L, B, R, F)
    out += fellipse(12, 14, 5.5, 2.2, op="0.7") + ellipse(12, 14, 5.5, 2.2)
    out += ellipse(12, 4.5, 5.5, 2.2) + line((6.5, 4.5), (6.5, 14)) + line((17.5, 4.5), (17.5, 14))
    return out


@icon("dimension")
def _():
    A, Bp = (7, 21), (21, 10.5)
    out = line(A, Bp) + dot(*A, 1.4) + dot(*Bp, 1.4)
    n = (-0.6, -0.8)
    a2, b2 = add(A, mul(n, 6.5)), add(Bp, mul(n, 6.5))
    out += line(add(A, mul(n, 2)), add(A, mul(n, 8)), stroke_width="1.2")
    out += line(add(Bp, mul(n, 2)), add(Bp, mul(n, 8)), stroke_width="1.2")
    out += arrow(lerp(a2, b2, 0.5), a2, s=2.2) + arrow(lerp(a2, b2, 0.5), b2, s=2.2)
    return out


@icon("three-point-arc")
def _():
    c = (12, 17)
    return arc(12, 17, 9, 9, 190, 350) + "".join(dot(*polar(c, 9, a), 1.9) for a in (190, 270, 350))


@icon("elliptical-arc")
def _():
    out = arc(12, 13, 9, 6.5, 180, 360) + arc(12, 13, 9, 6.5, 0, 180, stroke_dasharray=DASH)
    return out + dot(3, 13, 1.6) + dot(21, 13, 1.6) + dot(12, 13, 1.3)


# ---------------- constraints ----------------
@icon("pierce")
def _():
    L, B, R, F = (2, 15), (11, 10.5), (20, 15), (11, 19.5)
    out = fpoly(L, B, R, F, op="0.2") + poly(L, B, R, F)
    top, hit = (17, 2.5), (11, 15)
    d = norm(sub(hit, top))
    out += line(top, hit) + dline(hit, add(hit, mul(d, 6.5)))
    return out + dot(*hit, 2)


@icon("midpoint")
def _():
    out = line((3, 12), (21, 12)) + dot(3, 12, 1.3) + dot(21, 12, 1.3)
    out += circle(12, 12, 3) + dot(12, 12, 1.3)
    out += line((7, 9.5), (7, 14.5)) + line((17, 9.5), (17, 14.5))
    return out


@icon("curvature")
def _():
    a = ((3, 20), (5, 9), (12, 5), (21, 5))
    out = P(C(*a))
    env = []
    N = 9
    ks = []
    for i in range(N + 1):
        t = i / N
        h = 1e-3
        d1 = bezt(*a, t)
        d2 = sub(bezt(*a, min(t + h, 1)), bezt(*a, max(t - h, 0)))
        d2 = mul(d2, 1 / (2 * h if 0 < t < 1 else h))
        k = (d1[0] * d2[1] - d1[1] * d2[0]) / (math.hypot(*d1) ** 3)
        ks.append(abs(k))
    mx = max(ks)
    for i in range(N + 1):
        t = i / N
        p = bez(*a, t)
        d = norm(bezt(*a, t))
        n = (d[1], -d[0])
        q = add(p, mul(n, 0.8 + 6.5 * ks[i] / mx))
        env.append(q)
        out += line(p, q, stroke_width="1")
    out += P(smooth(env), stroke_width="1")
    return out


@icon("normal")
def _():
    out = arc(12, 25, 13, 13, 215, 325)
    out += line((12, 12), (12, 2.5)) + line((12, 8.8), (15.2, 8.8), (15.2, 12.4))
    return out + dot(12, 12, 1.5)


# ---------------- assembly ----------------
@icon("explode")
def _():
    o = center_o(10, 10, 11.5, 12, 12)
    base = Box(o, 10, 10, 3)
    top = Box(o, 5, 5, 4, x0=2.5, y0=2.5, z0=7.5)
    p = lambda x, y, z: iso(o, x, y, z)
    out = base.edges() + fpoly(*top.top()) + top.edges()
    out += dline(p(7.5, 2.5, 7.5), p(7.5, 2.5, 3.8)) + dline(p(2.5, 7.5, 7.5), p(2.5, 7.5, 3.8)) + dline(p(7.5, 7.5, 7.5), p(7.5, 7.5, 3.8))
    return out


@icon("mate-connector-assembly")
def _():
    s, b = cube(12, 14.5, 7.5)
    c = b.center("top")
    out = s + fellipse(c[0], c[1], 4, 1.9, op="0.6") + ellipse(c[0], c[1], 4, 1.9) + dot(*c, 1.2)
    return out + arrow(c, (c[0], 1.8), s=2.4)


@icon("fastened")
def _():
    out = fellipse(12, 15.5, 7.5, 3) + ellipse(12, 15.5, 7.5, 3) + ellipse(12, 11, 7.5, 3)
    out += line((4.5, 11), (4.5, 15.5)) + line((19.5, 11), (19.5, 15.5))
    out += rect(9.5, 2.5, 5, 3.2, 0.8) + line((12, 5.7), (12, 21.5)) + line((9.8, 21.5), (14.2, 21.5))
    return out


# ---------------- app ----------------
@icon("assembly")
def _():
    o = center_o(11, 11, 10, 12, 12.5)
    b = Box(o, 11, 11, 3.5)
    out = b.edges()
    c = iso(o, 5.5, 5.5, 10)
    c0 = iso(o, 5.5, 5.5, 3.5)
    rx, ry = 3.2 * 1.2247, 3.2 * 0.7071
    out += fellipse(c[0], c[1], rx, ry) + ellipse(c[0], c[1], rx, ry)
    out += line((c[0] - rx, c[1]), (c0[0] - rx, c0[1])) + line((c[0] + rx, c[1]), (c0[0] + rx, c0[1]))
    out += arc(c0[0], c0[1], rx, ry, 0, 180)
    return out


def _view_dir(face, ad, cx):
    s, b = cube(cx, 12.5, 6.6, fill=(face,))
    c = b.center(face)
    tip = add(c, mul(ad, 3.8))
    return s + arrow(add(tip, mul(ad, 5)), tip, s=2.3)


@icon("front-view")
def _():
    return _view_dir("left", (-C30, 0.5), 13.5)


@icon("right-view")
def _():
    return _view_dir("right", (C30, 0.5), 10.5)


@icon("top-view")
def _():
    s, b = cube(12, 14.5, 7, fill=("top",))
    return s + arrow((12, 1.5), (12, 5.8), s=2.3)


@icon("version-graph")
def _():
    out = line((5, 6.2), (5, 17.8))
    out += P("M5 15 Q5 12 8.5 12 H15.8") + P("M5 8.5 Q5 5 8.5 5 H15.8") if False else ""
    out += P("M5 15.2 Q5 12 8.5 12 H15.8")
    for c in ((5, 4), (5, 20), (18, 12)):
        out += circle(*c, 2.2)
    out += P("M18 9.8 V8 Q18 4 14 4 H7.2")
    return out


@icon("wire-body")
def _():
    o = center_o(9, 9, 9)
    p = lambda x, y, z: iso(o, x, y, z)
    a, b2, e = p(0, 9, 0), p(9, 9, 0), p(2, 0, 6.5)
    out = P(f"M{pt(a)} L{pt(b2)} C{pt(p(9, 9, 6))} {pt(p(9, 9, 9))} {pt(p(9, 4.5, 9))} "
            f"C{pt(p(9, 0, 9))} {pt(p(4.5, 0, 9))} {pt(e)}")
    return out + dot(*a, 1.8) + dot(*e, 1.8) + dot(*b2, 1.3)


@icon("perspective")
def _():
    VP = (22, 3)
    F = [(2.5, 9), (12.5, 9), (12.5, 19.5), (2.5, 19.5)]
    Bk = [lerp(q, VP, 0.42) for q in F]
    out = poly(*F)
    out += line(F[0], Bk[0], Bk[1], Bk[2], F[2]) + line(F[1], Bk[1])
    out += dline(Bk[1], VP) if False else ""
    return out + dot(*VP, 1.3) + dline(Bk[1], add(VP, (-1.2, 0.8)), stroke_width="1")


@icon("section-plane")
def _():
    o = center_o(6, 11, 10, 13, 12)
    b = Box(o, 6, 11, 10)
    p = lambda x, y, z: iso(o, x, y, z)
    out = fpoly(*b.right(), op="0.5") + b.edges()
    e = 2.4
    pl = [p(6, -e, -e), p(6, 11 + e, -e), p(6, 11 + e, 10 + e), p(6, -e, 10 + e)]
    out += poly(*pl, stroke_width="1.2")
    return out


# ---------------- drawing ----------------
@icon("auxiliary-view")
def _():
    base = [(3, 21), (3, 11), (8.5, 11), (14, 16.5), (14, 21)]
    out = fpoly(*base) + poly(*base)
    m = (11.25, 13.75)
    nrm = (0.7071, -0.7071)
    c = add(m, mul(nrm, 7.5))
    g = rect(-3.9, -1.9, 7.8, 3.8, 0.3)
    out += f'<g transform="translate({f(c[0])} {f(c[1])}) rotate(45)">{g}</g>'
    out += dline(add((8.5, 11), mul(nrm, 1.5)), add((8.5, 11), mul(nrm, 5.2)), stroke_width="1")
    out += dline(add((14, 16.5), mul(nrm, 1.5)), add((14, 16.5), mul(nrm, 5.2)), stroke_width="1")
    return out


@icon("detail-view")
def _():
    out = line((2, 15.5), (6.5, 15.5), (6.5, 21.5))
    out += circle(6.5, 15.5, 3.4, stroke_width="1.2")
    out += circle(15.5, 8.5, 6.5)
    out += line((10.2, 6.5), (16.5, 6.5), (16.5, 14.8))
    out += line((9, 13), (10.6, 11.6), stroke_width="1.2")
    return out


# ---------------- round 2 ----------------
GHOST = dict(stroke_opacity="0.45", stroke_width="1.3")
REF2 = dict(stroke_width="1.2", stroke_opacity="0.6")


@icon("polygon-inscribed")
def _():
    return circle(12, 12, 9, **REF2) + poly(*_ngon((12, 12), 9)) + dot(12, 12, 1.4)


@icon("polygon-circumscribed")
def _():
    return circle(12, 12, 7.4, **REF2) + poly(*_ngon((12, 12), 7.4 / C30, phase=0)) + dot(12, 12, 1.4)


@icon("view-cube")
def _():
    s, b = cube(12, 12, 9.5)
    out = s
    for face, op in (("top", "0.65"), ("left", "0.3"), ("right", "0.3")):
        ps = getattr(b, face)()
        c = (sum(q[0] for q in ps) / 4, sum(q[1] for q in ps) / 4)
        out += fpoly(*[lerp(c, q, 0.6) for q in ps], op=op)
    return out


@icon("transform")
def _():
    g, _ = cube(7, 16.5, 5.2, **GHOST)
    s, _ = cube(17, 7.5, 5.2, fill=("top", "left", "right"))
    return g + s + P("M11.5 19 Q17 19 17 15") + head((17, 15), (0, -1), 2.4)


@icon("isolate")
def _():
    g1, _ = cube(4.4, 12, 3.4, **GHOST)
    g2, _ = cube(19.6, 12, 3.4, **GHOST)
    s, _ = cube(12, 12, 5.4, fill=("top", "left", "right"))
    return g1 + g2 + s


def eye_glyph(cx, cy, w, h, pupil):
    d = (f"M{f(cx-w)} {f(cy)} Q{f(cx)} {f(cy-h*2)} {f(cx+w)} {f(cy)} "
         f"Q{f(cx)} {f(cy+h*2)} {f(cx-w)} {f(cy)} Z")
    return P(d) + circle(cx, cy, pupil)


@icon("hide")
def _():
    s, _ = cube(10, 9.5, 7.2, **GHOST)
    return s + eye_glyph(17.5, 18, 4.8, 2.3, 1.3) + line((13.5, 22), (21.5, 14))


@icon("show")
def _():
    s, _ = cube(10, 9.5, 7.2)
    return s + eye_glyph(17.5, 18, 4.8, 2.3, 1.3)


@icon("unlink")
def _():
    g = rect(1.5, 8.5, 9, 7, 3.5) + rect(13.5, 8.5, 9, 7, 3.5)
    g = f'<g transform="rotate(-45 12 12)">{g}</g>'
    return g + line((14.5, 14.5), (17, 17)) + line((9.5, 9.5), (7, 7)) if False else \
        g + line((15.2, 15.2), (17.6, 17.6), stroke_width="1.3") + line((8.8, 8.8), (6.4, 6.4), stroke_width="1.3")


@icon("perspective")
def _():
    VP = (21.5, 2.5)
    F = [(2.5, 9.5), (12.5, 9.5), (12.5, 20), (2.5, 20)]
    Bk = [lerp(q, VP, 0.4) for q in F]
    out = poly(*F) + line(F[0], Bk[0], Bk[1], Bk[2], F[2]) + line(F[1], Bk[1])
    out += P(M([Bk[0], VP]) + " " + M([Bk[2], VP]) + " " + M([Bk[1], VP]), stroke_width="1", stroke_opacity="0.6")
    return out


@icon("section-plane")
def _():
    o = center_o(6, 11, 10, 13, 11.5)
    b = Box(o, 6, 11, 10)
    p = lambda x, y, z: iso(o, x, y, z)
    out = fpoly(*b.right(), op="0.5") + b.edges()
    e = 2
    pl = [p(6, -e, -e), p(6, 11 + e, -e), p(6, 11 + e, 10 + e), p(6, -e, 10 + e)]
    return out + poly(*pl, stroke_width="1.2")


@icon("curvature")
def _():
    a = ((5, 21), (6.5, 11), (13, 7.5), (21.5, 7.5))
    out = P(C(*a))
    env = []
    N = 9
    ks = []
    for i in range(N + 1):
        t = i / N
        h = 1e-3
        d1 = bezt(*a, t)
        t0, t1 = max(t - h, 0), min(t + h, 1)
        d2 = mul(sub(bezt(*a, t1), bezt(*a, t0)), 1 / (t1 - t0))
        ks.append(abs((d1[0] * d2[1] - d1[1] * d2[0]) / (math.hypot(*d1) ** 3)))
    mx = max(ks)
    for i in range(N + 1):
        t = i / N
        p = bez(*a, t)
        d = norm(bezt(*a, t))
        q = add(p, mul((d[1], -d[0]), 0.8 + 5.5 * ks[i] / mx))
        env.append(q)
        out += line(p, q, stroke_width="1")
    return out + P(smooth(env), stroke_width="1")


@icon("curvature")
def _():
    a = ((6.5, 21.5), (6.5, 13), (11, 9.5), (21.5, 9.5))
    out = P(C(*a))
    N = 8
    ts = [0.03 + 0.94 * i / N for i in range(N + 1)]
    ks = []
    for t in ts:
        h = 1e-3
        d1 = bezt(*a, t)
        d2 = mul(sub(bezt(*a, t + h), bezt(*a, t - h)), 1 / (2 * h))
        ks.append(abs(d1[0] * d2[1] - d1[1] * d2[0]) / (math.hypot(*d1) ** 3))
    mx = max(ks)
    env = []
    for t, k in zip(ts, ks):
        p = bez(*a, t)
        d = norm(bezt(*a, t))
        q = add(p, mul((d[1], -d[0]), 0.6 + 6 * k / mx))
        env.append(q)
        out += line(p, q, stroke_width="1")
    return out + P(smooth(env), stroke_width="1")


@icon("tab")
def _():
    o = center_o(15, 10, 1.6, 12, 12.5)
    poly_xy = [(0, 0), (10, 0), (10, 3), (15, 3), (15, 7), (10, 7), (10, 10), (0, 10)]
    tab = [iso(o, 10, 3, 1.6), iso(o, 15, 3, 1.6), iso(o, 15, 7, 1.6), iso(o, 10, 7, 1.6)]
    return fpoly(*tab) + slab(o, poly_xy, 1.6)


@icon("section-plane")
def _():
    o = center_o(6, 11, 10, 13, 11)
    b = Box(o, 6, 11, 10)
    p = lambda x, y, z: iso(o, x, y, z)
    out = fpoly(*b.right(), op="0.5") + b.edges()
    e = 2
    pl = [p(6, -e, -e), p(6, 11 + e, -e), p(6, 11 + e, 10 + e), p(6, -e, 10 + e)]
    return out + poly(*pl, stroke_width="1.2")
