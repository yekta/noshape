"""The app icon SVGs from the in-app logo (crates/app/assets/icons/noshape-logo.svg, the mark at 2.5 + 19/24 of
the 24-unit canvas): python3 -I gen.py <logo.svg> <outdir>. build.sh renders and packs them."""
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from corner import figma_tile  # noqa: E402

src, out = sys.argv[1], sys.argv[2]
LOGO = open(src).read()
D = re.search(r' d="([^"]+)"', LOGO).group(1)

LIFT = 0.7  # of 24 units: the triangle's visual centre is below its box centre


def mark(x, y, size):
    """The mark with its 24-unit box at (x, y), `size` wide (the logo's path is that box at 2.5, 19 units)."""
    s = size / 19
    return f'<path fill="#000" fill-rule="evenodd" transform="translate({x - 2.5 * s:.4f} {y - 2.5 * s:.4f}) scale({s:.5f})" d="{D}"/>'


def svg(view, body, defs=""):
    return f'<svg xmlns="http://www.w3.org/2000/svg" width="{view}" height="{view}" viewBox="0 0 {view} {view}">{defs}{body}</svg>\n'


def write(name, text):
    open(f"{out}/{name}", "w").write(text)


# Favicon: the mark on a white square, small padding so it stays legible at 16 px.
write("favicon.svg", svg(32, '<rect width="32" height="32" fill="#fff"/>' + mark(4, 4, 24)))

# macOS: Apple's grid: an 824 tile at 100 on the 1024 canvas, continuous corners (r 185.4), the template's
# drop shadow (0 10 10, black 30 %); the mark MAC of the tile's width, optically centred.
MAC = 0.72
tile = figma_tile(100, 100, 824, 185.4, 0.6)
shadow = ('<defs><filter id="s" x="-20%" y="-20%" width="140%" height="140%">'
          '<feGaussianBlur in="SourceAlpha" stdDeviation="10"/><feOffset dy="10"/>'
          '<feComponentTransfer><feFuncA type="linear" slope="0.3"/></feComponentTransfer>'
          '<feMerge><feMergeNode/><feMergeNode in="SourceGraphic"/></feMerge></filter></defs>')
m = 824 * MAC
write("macos.svg", svg(1024, f'<path d="{tile}" fill="#fff" filter="url(#s)"/>' + mark(512 - m / 2, 512 - m / 2 - LIFT * m / 24, m), shadow))


# Windows / Linux: a white rounded tile with a faint edge (it holds on a light taskbar), black mark.
def tile_svg(view, inset, radius, glyph, edge, lift=0):
    w = view - 2 * inset
    e = edge / 2
    return svg(view, f'<rect x="{inset + e:g}" y="{inset + e:g}" width="{w - edge:g}" height="{w - edge:g}" rx="{radius:g}" fill="#fff" '
                     f'stroke="#000" stroke-opacity="0.18" stroke-width="{edge:g}"/>' + mark((view - glyph) / 2, (view - glyph) / 2 - lift * glyph / 24, glyph))


write("tile-256.svg", tile_svg(256, 8, 48, 240 * MAC, 2, LIFT))
write("tile-48.svg", tile_svg(48, 1, 9, 32, 1))
write("tile-32.svg", tile_svg(32, 0, 6, 23, 1))
write("tile-24.svg", tile_svg(24, 0, 4, 18, 1))
# 16 px: the favicon's art (a white square, the mark as large as it reads).
write("tile-16.svg", svg(16, '<rect width="16" height="16" fill="#fff"/>' + mark(2, 2, 12)))
