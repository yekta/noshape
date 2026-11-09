#!/bin/bash
# Rebuilds every app icon from the in-app logo, crates/app/assets/icons/noshape-logo.svg (the one source of the mark):
# packaging/icons/{noshape.svg, favicon.svg, noshape-256.png, noshape-512.png, noshape.ico, noshape.icns}.
# Needs python3, rsvg-convert, ImageMagick (magick), png2icns (icnsutils) and oxipng. PNGs stay 8-bit RGBA (oxipng --nx):
# macOS ignores an .icns whose PNGs are grayscale and boxes the app in a grey tile (11-09).
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
icons=packaging/icons
t=$(mktemp -d); trap 'rm -rf "$t"' EXIT
python3 -I "$here/gen.py" crates/app/assets/icons/noshape-logo.svg "$t"
png() { rsvg-convert -w "$2" -h "$2" "$t/$1.svg" -o "$3"; oxipng -q -o 4 --strip safe --nx "$3"; }

cp "$t/tile-256.svg" "$icons/noshape.svg"
cp "$t/favicon.svg" "$icons/favicon.svg"
png tile-256 256 "$icons/noshape-256.png"
png tile-256 512 "$icons/noshape-512.png"

# Windows: 16, 24, 32 and 48 px drawn for their size, 64 and 256 from the large tile.
for s in 16 24 32 48; do png "tile-$s" "$s" "$t/ico-$s.png"; done
for s in 64 256; do png tile-256 "$s" "$t/ico-$s.png"; done
magick "$t"/ico-{16,24,32,48,64,256}.png "$icons/noshape.ico"

# macOS: the 16 / 32 px legacy entries (png2icns) and PNG entries for 32 to 1024 px, @2x included.
for s in 16 32 64 128 256 512 1024; do png macos "$s" "$t/mac-$s.png"; done
png2icns "$t/legacy.icns" "$t/mac-16.png" "$t/mac-32.png" > /dev/null
python3 -I - "$t" "$icons/noshape.icns" <<'EOF'
import struct, sys
t, out = sys.argv[1], sys.argv[2]
b = open(f"{t}/legacy.icns", "rb").read()
chunks, i = [], 8
while i < len(b):
    kind, n = struct.unpack(">4sI", b[i:i + 8])
    chunks.append(b[i:i + n]); i += n
for kind, size in [("ic07", 128), ("ic08", 256), ("ic09", 512), ("ic11", 32), ("ic12", 64), ("ic13", 256), ("ic14", 512), ("ic10", 1024)]:
    data = open(f"{t}/mac-{size}.png", "rb").read()
    chunks.append(kind.encode() + struct.pack(">I", 8 + len(data)) + data)
body = b"".join(chunks)
open(out, "wb").write(b"icns" + struct.pack(">I", 8 + len(body)) + body)
EOF
echo "icons rebuilt from crates/app/assets/icons/noshape-logo.svg"
