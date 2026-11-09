import math
def figma_tile(x, y, w, r, xi=0.6):
    """Continuous-corner rounded square (Figma's corner smoothing; iOS / macOS use ~60%)."""
    p = (1 + xi) * r
    arc_deg = 90 * (1 - xi)
    arc = math.sin(math.radians(arc_deg / 2)) * r * math.sqrt(2)
    alpha = (90 - arc_deg) / 2
    p34 = r * math.tan(math.radians(alpha / 2))
    beta = 45 * xi
    c = p34 * math.cos(math.radians(beta))
    d = c * math.tan(math.radians(beta))
    b = (p - arc - c - d) / 3
    a = 2 * b
    f = lambda v: f"{v:.3f}".rstrip("0").rstrip(".")
    s = [f"M{f(x + w - p)} {f(y)}",
         f"c{f(a)} 0 {f(a + b)} 0 {f(a + b + c)} {f(d)}", f"a{f(r)} {f(r)} 0 0 1 {f(arc)} {f(arc)}", f"c{f(d)} {f(c)} {f(d)} {f(b + c)} {f(d)} {f(a + b + c)}",
         f"L{f(x + w)} {f(y + w - p)}",
         f"c0 {f(a)} 0 {f(a + b)} {f(-d)} {f(a + b + c)}", f"a{f(r)} {f(r)} 0 0 1 {f(-arc)} {f(arc)}", f"c{f(-c)} {f(d)} {f(-(b + c))} {f(d)} {f(-(a + b + c))} {f(d)}",
         f"L{f(x + p)} {f(y + w)}",
         f"c{f(-a)} 0 {f(-(a + b))} 0 {f(-(a + b + c))} {f(-d)}", f"a{f(r)} {f(r)} 0 0 1 {f(-arc)} {f(-arc)}", f"c{f(-d)} {f(-c)} {f(-d)} {f(-(b + c))} {f(-d)} {f(-(a + b + c))}",
         f"L{f(x)} {f(y + p)}",
         f"c0 {f(-a)} 0 {f(-(a + b))} {f(d)} {f(-(a + b + c))}", f"a{f(r)} {f(r)} 0 0 1 {f(arc)} {f(-arc)}", f"c{f(c)} {f(-d)} {f(b + c)} {f(-d)} {f(a + b + c)} {f(-d)}", "Z"]
    return "".join(s)
