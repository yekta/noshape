"""Our own macOS-style cursor art (noshape): move, nwse/nesw resize, wait.
Drawn from geometry; rendered at 1x/2x/3x of a 24 pt canvas."""
import math, subprocess, sys
from PIL import Image

SHADOW = '<defs><filter id="s" x="-30%" y="-30%" width="160%" height="160%"><feDropShadow dx="0" dy="0.8" stdDeviation="0.7" flood-color="#000" flood-opacity="0.38"/></filter></defs>'
def wrap(body, side=24):
    return f'<svg xmlns="http://www.w3.org/2000/svg" width="{side}" height="{side}" viewBox="0 0 {side} {side}">{SHADOW}<g filter="url(#s)">{body}</g></svg>'

def outlined(d):
    # black body, 1.5 pt white outline outside it (stroke under the fill)
    return f'<path d="{d}" fill="#fff" stroke="#fff" stroke-width="2" stroke-linejoin="round"/><path d="{d}" fill="#000"/>'

# Move: four arrows on a cross; heads 7 wide x 4.2 long, shafts 2 wide.
def move():
    c, L, hw, hl, sw = 12, 10.2, 3.7, 4.2, 0.95
    pts = []
    # start at top tip, go clockwise
    for k in range(4):
        a = k * math.pi / 2
        def P(x, y):  # local frame: arrow pointing "up" (−y)
            ca, sa = math.cos(a), math.sin(a)
            return (c + x * ca - y * sa, c + x * sa + y * ca)
        pts += [P(-sw, -L + hl), P(-hw, -L + hl), P(0, -L), P(hw, -L + hl), P(sw, -L + hl), P(sw, -sw)]
    d = 'M' + ' L'.join(f'{x:.3f} {y:.3f}' for x, y in pts) + ' Z'
    return wrap(outlined(d))

# Diagonal resize: a shaft with a right-angled triangular head at each end.
def resize(nesw):
    c, half, head, sw = 12, 8.4, 6.0, 0.95
    # nwse along (1,1); build in a frame where the axis is +x, then rotate 45°
    pts_local = [(-half, 0), (-half + head * 0.72, -head * 0.72), (-half + head * 0.72, -sw), (half - head * 0.72, -sw),
                 (half - head * 0.72, -head * 0.72), (half, 0), (half - head * 0.72, head * 0.72), (half - head * 0.72, sw),
                 (-half + head * 0.72, sw), (-half + head * 0.72, head * 0.72)]
    a = -math.pi / 4 if nesw else math.pi / 4
    ca, sa = math.cos(a), math.sin(a)
    pts = [(c + x * ca - y * sa, c + x * sa + y * ca) for x, y in pts_local]
    d = 'M' + ' L'.join(f'{x:.3f} {y:.3f}' for x, y in pts) + ' Z'
    return wrap(outlined(d))

# Wait (Chrome on macOS: busy-but-clickable): the arrow and a pinwheel.
ARROW = "M4 3 L4 17.6 L7.6 14.2 L10.1 19.9 C10.35 20.4 10.9 20.55 11.4 20.3 C11.85 20.05 12 19.5 11.8 19.05 L9.4 13.5 L14.3 13.5 Z"
def wait():
    cx, cy, r = 17.4, 17.6, 4.7
    cols = ['#2f8cf3', '#8a4fe0', '#ec3a5b', '#f5871f', '#f7cf25', '#4cc54a']
    segs = []
    for i, col in enumerate(cols):
        a0 = math.radians(-90 + i * 60)
        a1 = a0 + math.radians(60)
        # swirl: the boundary bows (quadratic via a control point rotated +25°)
        def edge(a):
            return (cx + r * math.cos(a), cy + r * math.sin(a))
        cm = math.radians(25)
        q0 = (cx + r * 0.55 * math.cos(a0 - cm), cy + r * 0.55 * math.sin(a0 - cm))
        q1 = (cx + r * 0.55 * math.cos(a1 - cm), cy + r * 0.55 * math.sin(a1 - cm))
        p0, p1 = edge(a0), edge(a1)
        segs.append(f'<path d="M{cx} {cy} Q{q0[0]:.3f} {q0[1]:.3f} {p0[0]:.3f} {p0[1]:.3f} A{r} {r} 0 0 1 {p1[0]:.3f} {p1[1]:.3f} Q{q1[0]:.3f} {q1[1]:.3f} {cx} {cy} Z" fill="{col}"/>')
    shine = f'<circle cx="{cx}" cy="{cy}" r="{r}" fill="url(#g)"/>'
    grad = '<radialGradient id="g" cx="0.38" cy="0.3" r="0.75"><stop offset="0" stop-color="#fff" stop-opacity="0.45"/><stop offset="0.6" stop-color="#fff" stop-opacity="0"/><stop offset="1" stop-color="#000" stop-opacity="0.18"/></radialGradient>'
    ring = f'<circle cx="{cx}" cy="{cy}" r="{r + 0.35}" fill="#000" fill-opacity="0.3"/>'
    body = outlined(ARROW) + ring + ''.join(segs) + shine
    return wrap(f'<defs>{grad}</defs>' + body)

ART = {'move': move(), 'resize-nwse': resize(False), 'resize-nesw': resize(True), 'wait': wait()}
out = sys.argv[1]
for name, svg in ART.items():
    open(f'{name}.svg', 'w').write(svg)
    for k, suf in [(1, ''), (2, '@2x'), (3, '@3x')]:
        subprocess.run(['resvg', '-w', str(24 * k), '-h', str(24 * k), f'{name}.svg', f'{out}/{name}{suf}.png'], check=True)
