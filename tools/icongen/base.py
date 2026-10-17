import math

ICONS = {}


def icon(*names):
    def deco(fn):
        for n in names:
            ICONS[n] = fn
        return fn
    return deco


def f(v):
    s = f"{v:.2f}".rstrip("0").rstrip(".")
    return "0" if s in ("-0", "") else s


def attrs(a):
    out = ""
    for k, v in a.items():
        if v is None:
            continue
        k = k.rstrip("_").replace("_", "-")
        out += f' {k}="{v}"'
    return out


HLOP = "0.35"
DASH = "1.2 2.8"          # visible dash ~2.7, gap ~1.3 with round caps
DASHDOT = "5 2.6 0.01 2.6"


def pt(p):
    return f"{f(p[0])} {f(p[1])}"


def M(ps, close=False):
    d = "M" + " L".join(pt(p) for p in ps)
    return d + (" Z" if close else "")


def P(d, **a):
    return f'<path d="{d}"{attrs(a)}/>'


def line(*ps, **a):
    return P(M(ps), **a)


def dline(*ps, **a):
    return P(M(ps), stroke_dasharray=DASH, **a)


def ddline(*ps, **a):
    return P(M(ps), stroke_dasharray=DASHDOT, **a)


def poly(*ps, **a):
    return P(M(ps, True), **a)


def fpoly(*ps, op=HLOP):
    return P(M(ps, True), fill="black", fill_opacity=op, stroke="none")


def fpath(d, op=HLOP, **a):
    return P(d, fill="black", fill_opacity=op, stroke="none", **a)


def circle(cx, cy, r, **a):
    return f'<circle cx="{f(cx)}" cy="{f(cy)}" r="{f(r)}"{attrs(a)}/>'


def fcircle(cx, cy, r, op=HLOP):
    return circle(cx, cy, r, fill="black", fill_opacity=op, stroke="none")


def dot(cx, cy, r=1.5):
    return circle(cx, cy, r, fill="black", stroke="none")


def ellipse(cx, cy, rx, ry, **a):
    return f'<ellipse cx="{f(cx)}" cy="{f(cy)}" rx="{f(rx)}" ry="{f(ry)}"{attrs(a)}/>'


def fellipse(cx, cy, rx, ry, op=HLOP):
    return ellipse(cx, cy, rx, ry, fill="black", fill_opacity=op, stroke="none")


def rect(x, y, w, h, rx=0, **a):
    r = f' rx="{f(rx)}"' if rx else ""
    return f'<rect x="{f(x)}" y="{f(y)}" width="{f(w)}" height="{f(h)}"{r}{attrs(a)}/>'


def frect(x, y, w, h, rx=0, op=HLOP):
    return rect(x, y, w, h, rx, fill="black", fill_opacity=op, stroke="none")


# ---------- vectors ----------
def add(a, b):
    return (a[0] + b[0], a[1] + b[1])


def sub(a, b):
    return (a[0] - b[0], a[1] - b[1])


def mul(a, s):
    return (a[0] * s, a[1] * s)


def norm(a):
    l = math.hypot(*a) or 1
    return (a[0] / l, a[1] / l)


def lerp(a, b, t):
    return (a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t)


def rot(v, deg):
    r = math.radians(deg)
    return (v[0] * math.cos(r) - v[1] * math.sin(r), v[0] * math.sin(r) + v[1] * math.cos(r))


def polar(c, r, deg, ry=None):
    ry = r if ry is None else ry
    t = math.radians(deg)
    return (c[0] + r * math.cos(t), c[1] + ry * math.sin(t))


# ---------- arrows ----------
def head(tip, d, s=2.6, ang=42, **a):
    u = norm((-d[0], -d[1]))
    w1 = add(tip, mul(rot(u, ang), s))
    w2 = add(tip, mul(rot(u, -ang), s))
    return line(w1, tip, w2, **a)


def arrow(p1, p2, s=2.6, both=False, **a):
    out = line(p1, p2, **a) + head(p2, sub(p2, p1), s)
    if both:
        out += head(p1, sub(p1, p2), s)
    return out


def arcd(cx, cy, rx, ry, a0, a1, move=True):
    p0 = polar((cx, cy), rx, a0, ry)
    p1 = polar((cx, cy), rx, a1, ry)
    large = 1 if abs(a1 - a0) > 180 else 0
    sweep = 1 if a1 > a0 else 0
    d = (f"M{pt(p0)} " if move else "") + f"A{f(rx)} {f(ry)} 0 {large} {sweep} {pt(p1)}"
    return d


def arc(cx, cy, rx, ry, a0, a1, **a):
    return P(arcd(cx, cy, rx, ry, a0, a1), **a)


def arc_tangent(rx, ry, t, sign):
    r = math.radians(t)
    return (-rx * math.sin(r) * sign, ry * math.cos(r) * sign)


def arcarrow(cx, cy, rx, ry, a0, a1, s=2.6, both=False, **a):
    sign = 1 if a1 > a0 else -1
    out = arc(cx, cy, rx, ry, a0, a1, **a)
    out += head(polar((cx, cy), rx, a1, ry), arc_tangent(rx, ry, a1, sign), s)
    if both:
        out += head(polar((cx, cy), rx, a0, ry), arc_tangent(rx, ry, a0, -sign), s)
    return out


# ---------- bezier ----------
def bez(p0, p1, p2, p3, t):
    u = 1 - t
    return (u**3 * p0[0] + 3 * u * u * t * p1[0] + 3 * u * t * t * p2[0] + t**3 * p3[0],
            u**3 * p0[1] + 3 * u * u * t * p1[1] + 3 * u * t * t * p2[1] + t**3 * p3[1])


def bezt(p0, p1, p2, p3, t):
    u = 1 - t
    return (3 * u * u * (p1[0] - p0[0]) + 6 * u * t * (p2[0] - p1[0]) + 3 * t * t * (p3[0] - p2[0]),
            3 * u * u * (p1[1] - p0[1]) + 6 * u * t * (p2[1] - p1[1]) + 3 * t * t * (p3[1] - p2[1]))


def C(p0, p1, p2, p3, move=True):
    return (f"M{pt(p0)} " if move else "") + f"C{pt(p1)} {pt(p2)} {pt(p3)}"


def Q(p0, p1, p2, move=True):
    return (f"M{pt(p0)} " if move else "") + f"Q{pt(p1)} {pt(p2)}"


def smooth(ps):
    """Catmull-Rom through points -> cubic bezier path."""
    d = f"M{pt(ps[0])}"
    for i in range(len(ps) - 1):
        p0 = ps[i - 1] if i > 0 else ps[i]
        p1 = ps[i]
        p2 = ps[i + 1]
        p3 = ps[i + 2] if i + 2 < len(ps) else p2
        c1 = add(p1, mul(sub(p2, p0), 1 / 6))
        c2 = sub(p2, mul(sub(p3, p1), 1 / 6))
        d += f" C{pt(c1)} {pt(c2)} {pt(p2)}"
    return d


# ---------- iso ----------
C30 = math.cos(math.radians(30))


def iso(o, x, y, z):
    return (o[0] + (x - y) * C30, o[1] + (x + y) * 0.5 - z)


def center_o(dx, dy, dz, cx=12, cy=12):
    return (cx - (dx - dy) * C30 / 2, cy - ((dx + dy) / 2 - dz) / 2)


class Box:
    def __init__(self, o, dx, dy, dz, x0=0, y0=0, z0=0):
        self.o = o
        self.x0, self.y0, self.z0 = x0, y0, z0
        self.x1, self.y1, self.z1 = x0 + dx, y0 + dy, z0 + dz

    def p(self, x, y, z):
        return iso(self.o, x, y, z)

    def outline(self):
        a, b, c, A, B, Cc = self.x0, self.y0, self.z0, self.x1, self.y1, self.z1
        p = self.p
        return [p(a, b, Cc), p(A, b, Cc), p(A, b, c), p(A, B, c), p(a, B, c), p(a, B, Cc)]

    def inner(self):
        a, b, c, A, B, Cc = self.x0, self.y0, self.z0, self.x1, self.y1, self.z1
        p = self.p
        n = p(A, B, Cc)
        return [(n, p(A, b, Cc)), (n, p(a, B, Cc)), (n, p(A, B, c))]

    def edges(self, **a):
        out = poly(*self.outline(), **a)
        d = " ".join(M([u, v]) for u, v in self.inner())
        return out + P(d, **a)

    def hidden(self, **a):
        a0, b, c, A, B, Cc = self.x0, self.y0, self.z0, self.x1, self.y1, self.z1
        p = self.p
        h = p(a0, b, c)
        d = " ".join(M([h, q]) for q in (p(A, b, c), p(a0, B, c), p(a0, b, Cc)))
        return P(d, stroke_dasharray=DASH, **a)

    def top(self):
        p = self.p
        return [p(self.x0, self.y0, self.z1), p(self.x1, self.y0, self.z1),
                p(self.x1, self.y1, self.z1), p(self.x0, self.y1, self.z1)]

    def right(self):  # x = x1 face
        p = self.p
        return [p(self.x1, self.y0, self.z0), p(self.x1, self.y1, self.z0),
                p(self.x1, self.y1, self.z1), p(self.x1, self.y0, self.z1)]

    def left(self):  # y = y1 face
        p = self.p
        return [p(self.x0, self.y1, self.z0), p(self.x1, self.y1, self.z0),
                p(self.x1, self.y1, self.z1), p(self.x0, self.y1, self.z1)]

    def center(self, face):
        ps = getattr(self, face)()
        return (sum(q[0] for q in ps) / 4, sum(q[1] for q in ps) / 4)


def cube(cx, cy, a, fill=(), **kw):
    o = center_o(a, a, a, cx, cy)
    b = Box(o, a, a, a)
    out = ""
    for face in fill:
        if isinstance(face, tuple):
            out += fpoly(*getattr(b, face[0])(), op=face[1])
        else:
            out += fpoly(*getattr(b, face)())
    return out + b.edges(**kw), b


def iso_ellipse(c, r):
    """horizontal circle radius r projected -> (rx, ry)"""
    return (c[0], c[1], r * 1.2247, r * 0.7071)


def prism(o, profile, D, axis="y", fill_cap=False, fill_faces=(), sharp=25,
          cap_op=HLOP, extra_y_edges=(), skip_back=()):
    """Extrude closed 2D profile [(s,z)] along axis ('y': s=x, cap at y=D;
    'x': s=y, cap at x=D). Returns svg with visible edges."""
    def P3(s, z, d):
        if axis == "y":
            return iso(o, s, d, z)
        return iso(o, d, s, z)

    n = len(profile)
    area = 0
    for i in range(n):
        a, b = profile[i], profile[(i + 1) % n]
        area += a[0] * b[1] - b[0] * a[1]
    ccw = area > 0
    vis = []
    dirs = []
    for i in range(n):
        a, b = profile[i], profile[(i + 1) % n]
        dx, dz = b[0] - a[0], b[1] - a[1]
        # outward normal
        nx, nz = (dz, -dx) if ccw else (-dz, dx)
        vis.append(nx + nz > 1e-6)
        dirs.append(math.atan2(dz, dx))
    out = ""
    # fills
    for i in fill_faces:
        a, b = profile[i], profile[(i + 1) % n]
        out += fpoly(P3(*a, D), P3(*b, D), P3(*b, 0), P3(*a, 0))
    capd = M([P3(s, z, D) for s, z in profile], True)
    if fill_cap:
        out += fpath(capd, op=cap_op)
    out += P(capd)
    d = []
    for i in range(n):
        prev = (i - 1) % n
        turn = abs((math.degrees(dirs[i] - dirs[prev]) + 180) % 360 - 180)
        silhouette = vis[i] != vis[prev]
        crease = vis[i] and vis[prev] and turn > sharp
        if silhouette or crease or i in extra_y_edges:
            s, z = profile[i]
            d.append(M([P3(s, z, D), P3(s, z, 0)]))
    for i in range(n):
        if vis[i] and i not in skip_back:
            a, b = profile[i], profile[(i + 1) % n]
            d.append(M([P3(*a, 0), P3(*b, 0)]))
    out += P(" ".join(d))
    return out


def arcpts(c, r, a0, a1, n=8):
    """2D arc points in (s,z) space, angles in math convention (z up)."""
    return [(c[0] + r * math.cos(math.radians(a0 + (a1 - a0) * i / n)),
             c[1] + r * math.sin(math.radians(a0 + (a1 - a0) * i / n))) for i in range(n + 1)]


# ---------- badges / common glyphs ----------
def badge_plus(cx=18.5, cy=18.5, s=3.2):
    return line((cx - s, cy), (cx + s, cy)) + line((cx, cy - s), (cx, cy + s))


def badge_minus(cx=18.5, cy=18.5, s=3.2):
    return line((cx - s, cy), (cx + s, cy))


def badge_x(cx=18.5, cy=18.5, s=2.6):
    return line((cx - s, cy - s), (cx + s, cy + s)) + line((cx - s, cy + s), (cx + s, cy - s))


def pencil(tip, angle=-45, L=13, w=4.2):
    """pencil whose tip is at `tip`, body extending in direction `angle` (deg, screen)."""
    u = (math.cos(math.radians(angle)), math.sin(math.radians(angle)))
    n = (-u[1], u[0])
    S = add(tip, mul(u, 3.6))
    B = add(tip, mul(u, L))
    hw = w / 2
    a = add(S, mul(n, hw)); b = add(B, mul(n, hw)); c = sub(B, mul(n, hw)); d = sub(S, mul(n, hw))
    E = add(tip, mul(u, L - 2.6))
    return (poly(tip, a, b, c, d) + line(a, d) +
            line(add(E, mul(n, hw)), sub(E, mul(n, hw))))


def disc(cx, cy, rx=5, ry=2.2, fill=False):
    out = fellipse(cx, cy, rx, ry) if fill else ""
    return out + ellipse(cx, cy, rx, ry) + dot(cx, cy, 1.1)


def gear(cx, cy, r, teeth=8, depth=1.6, phase=0):
    ps = []
    n = teeth * 4
    for i in range(n):
        a = phase + 360 * i / n
        k = i % 4
        rr = r if k in (0, 1) else r - depth
        ps.append(polar((cx, cy), rr, a))
    return poly(*ps)


def svg(body):
    return ('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" '
            'stroke="black" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">'
            + body + "</svg>\n")


def strip(o, pts, D, axis="y", fill_segs=(), crease=(), op=HLOP):
    """Open 2D profile [(s,z)] extruded along axis as a zero-thickness surface."""
    def P3(s, z, d):
        return iso(o, s, d, z) if axis == "y" else iso(o, d, s, z)
    out = ""
    for i in fill_segs:
        a, b = pts[i], pts[i + 1]
        out += fpoly(P3(*a, D), P3(*b, D), P3(*b, 0), P3(*a, 0), op=op)
    front = [P3(s, z, D) for s, z in pts]
    back = [P3(s, z, 0) for s, z in pts]
    d = [M(front), M(back), M([front[0], back[0]]), M([front[-1], back[-1]])]
    for i in crease:
        d.append(M([front[i], back[i]]))
    return out + P(" ".join(d))


def slab(o, poly_xy, t, fill_top=False, top_op=HLOP, fill_side=()):
    """Extrude a closed (x,y) polygon upward by t; top face visible."""
    def P3(x, y, z):
        return iso(o, x, y, z)
    n = len(poly_xy)
    area = sum(poly_xy[i][0] * poly_xy[(i + 1) % n][1] - poly_xy[(i + 1) % n][0] * poly_xy[i][1]
               for i in range(n))
    ccw = area > 0
    vis = []
    dirs = []
    for i in range(n):
        a, b = poly_xy[i], poly_xy[(i + 1) % n]
        dx, dy = b[0] - a[0], b[1] - a[1]
        nx, ny = (dy, -dx) if ccw else (-dy, dx)
        vis.append(nx + ny > 1e-6)
        dirs.append(math.atan2(dy, dx))
    out = ""
    for i in fill_side:
        a, b = poly_xy[i], poly_xy[(i + 1) % n]
        out += fpoly(P3(*a, t), P3(*b, t), P3(*b, 0), P3(*a, 0))
    top = M([P3(x, y, t) for x, y in poly_xy], True)
    if fill_top:
        out += fpath(top, op=top_op)
    out += P(top)
    d = []
    for i in range(n):
        prev = (i - 1) % n
        turn = abs((math.degrees(dirs[i] - dirs[prev]) + 180) % 360 - 180)
        if vis[i] != vis[prev] or (vis[i] and vis[prev] and turn > 25):
            x, y = poly_xy[i]
            d.append(M([P3(x, y, t), P3(x, y, 0)]))
    for i in range(n):
        if vis[i]:
            a, b = poly_xy[i], poly_xy[(i + 1) % n]
            d.append(M([P3(*a, 0), P3(*b, 0)]))
    out += P(" ".join(d))
    return out


def sheet(cx, cy, w=8, h=4.5, amp=1.6, amp2=1.2):
    """Wavy iso surface patch. Returns (path_d, data) where data has edge curves."""
    L = (cx - w, cy); F = (cx, cy + h); R = (cx + w, cy); B = (cx, cy - h)
    e1 = (L, add(lerp(L, F, 1 / 3), (0, -amp)), add(lerp(L, F, 2 / 3), (0, amp)), F)
    e2 = (F, add(lerp(F, R, 1 / 3), (0, -amp2)), add(lerp(F, R, 2 / 3), (0, -amp2)), R)
    t1 = sub(B, L)
    t2 = sub(L, F)
    e1b = tuple(add(p, t1) for p in e1)     # B -> R
    e2b = tuple(add(p, t2) for p in e2)     # L -> B
    d = (f"M{pt(L)} C{pt(e1[1])} {pt(e1[2])} {pt(F)} C{pt(e2[1])} {pt(e2[2])} {pt(R)} "
         f"C{pt(e1b[2])} {pt(e1b[1])} {pt(B)} C{pt(e2b[2])} {pt(e2b[1])} {pt(L)} Z")
    return d, dict(L=L, F=F, R=R, B=B, e1=e1, e2=e2, e1b=e1b, e2b=e2b)


def curve_d(c):
    return C(*c)


def shift(c, v):
    return tuple(add(p, v) for p in c)
