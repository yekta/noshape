# Weld joint / spacer icons for the Weld symbol dialog dropdowns (24x24,
# a reference line with the joint above it, both sides for spacers).
import math, sys, os
out = sys.argv[1]
S = 3.0
def arc(c, r, a0, a1, n=12):
    return [(c[0] + r*math.cos(a0 + (a1-a0)*i/n), c[1] + r*math.sin(a0 + (a1-a0)*i/n)) for i in range(n+1)]
def glyph(j, x, y, d):
    """Polylines (model units, y up) like weld.rs glyph()."""
    h = S*d; P = math.pi; L = []
    if j == 'square-groove': L += [[(x+.8,y),(x+.8,y+h)],[(x+2.2,y),(x+2.2,y+h)]]
    elif j == 'v-groove': L += [[(x,y+h),(x+1.5,y),(x+3,y+h)]]
    elif j == 'bevel-groove': L += [[(x,y+h),(x,y),(x+2,y+h)]]
    elif j == 'bevel-butt-broad-root': L += [[(x,y+h),(x,y),(x+.6,y),(x+.6,y+h*.4),(x+2.2,y+h)]]
    elif j == 'u-groove':
        L += [[(x+1.5,y),(x+1.5,y+h*.4)], arc((x+1.5,y+h*.75),1.2,P,2*P) if d>0 else arc((x+1.5,y+h*.75),1.2,0,P)]
    elif j == 'j-groove':
        L += [[(x,y),(x,y+h)], arc((x,y+h*.75),1.4,-P/2,0) if d>0 else arc((x,y+h*.75),1.4,0,P/2)]
    elif j == 'v-flare-groove':
        L += [arc((x-.2,y+h),1.5,-P/2 if d>0 else 0,0 if d>0 else P/2), arc((x+3.2,y+h),1.5,P if d>0 else P/2,1.5*P if d>0 else P)]
    elif j == 'bevel-flare-groove':
        L += [[(x,y),(x,y+h)], arc((x+2.6,y+h),1.6,P if d>0 else P/2,1.5*P if d>0 else P)]
    elif j == 'bead': L += [arc((x+1.5,y),1.5,0,P) if d>0 else arc((x+1.5,y),1.5,P,2*P)]
    elif j == 'fillet': L += [[(x,y),(x,y+h),(x+S,y)]]
    elif j == 'plug-slot': L += [[(x,y),(x,y+h*.8),(x+3,y+h*.8),(x+3,y)]]
    elif j in ('seam','spot'):
        cy = y + h*.5
        L += [arc((x+1.5,cy),1.4,0,2*P,24)]
        if j.startswith('seam'): L += [[(x-.4,cy+.5),(x+3.4,cy+.5)],[(x-.4,cy-.5),(x+3.4,cy-.5)]]
    return L
JOINTS = ['square-groove','v-groove','u-groove','j-groove','bevel-groove','bevel-butt-broad-root','v-flare-groove','bevel-flare-groove','bead','fillet','plug-slot','seam','spot']
SPACERS = {'double-v':'v-groove','double-bevel':'bevel-groove','double-u':'u-groove','double-j':'j-groove','double-flare-v':'v-flare-groove','double-flare-bevel':'bevel-flare-groove'}
def svg(lines, name, k=4.0, oy=12):
    # model: x 0..~4.5, y -3.5..3.5 -> 24px, scale k, origin (12 - 1.5*k, oy)
    ox = 12 - 1.5*k
    paths = []
    for l in lines:
        paths.append('M' + ' L'.join(f'{ox + p[0]*k:.2f} {oy - p[1]*k:.2f}' for p in l))
    body = ''.join(f'<path d="{p}"/>' for p in paths)
    s = f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.1" stroke-linejoin="round">{body}</svg>'
    open(os.path.join(out, f'weld-{name}.svg'), 'w').write(s)
for j in JOINTS:
    # the "other side" (above) icon; the arrow side icon is mirrored
    svg(glyph(j, 0, 0, 1), j)
    svg(glyph(j, 0, 0, -1), j + '-below')
for n, j in SPACERS.items():
    svg(glyph(j, 0, 0, 1) + glyph(j, 0, 0, -1), n)
# Dropdown list icons: the upright glyph, centred and larger.
for j in JOINTS:
    svg(glyph(j, 0, 0, 1), 'menu-' + j, 5.0, 12 + 1.5 * 5.0)
for n, j in SPACERS.items():
    svg(glyph(j, 0, 0, 1) + glyph(j, 0, 0, -1), 'menu-' + n, 3.2, 12)
