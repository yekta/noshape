#!/bin/bash
# zoom.sh out.png name1 name2 ...
out=$1; shift
SRC=/root/noshape/crates/app/assets/icons
args=()
for n in "$@"; do rsvg-convert -w 96 -h 96 -b white $SRC/$n.svg -o /tmp/icon-sheets/z_$n.png; args+=(/tmp/icon-sheets/z_$n.png); done
montage -label '%t' "${args[@]}" -tile 6x -geometry 96x96+8+4 -pointsize 11 $out
rm -f /tmp/icon-sheets/z_*.png
