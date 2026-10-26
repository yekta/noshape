#!/bin/bash
# usage: sheet.sh <names-file-or-glob-prefix> <outname>
set -e
SRC=/root/projects/noshape/crates/app/assets/icons
OUT=/tmp/icon-sheets
mkdir -p $OUT/png20 $OUT/png48
names=$(cat "$1")
args20=(); args48=()
for n in $names; do
  rsvg-convert -w 20 -h 20 -b white $SRC/$n.svg -o $OUT/png20/$n.png
  rsvg-convert -w 48 -h 48 -b white $SRC/$n.svg -o $OUT/png48/$n.png
  args20+=($OUT/png20/$n.png); args48+=($OUT/png48/$n.png)
done
montage -label '%t' "${args48[@]}" -tile 10x -geometry 48x48+10+6 -pointsize 9 $OUT/$2-48.png
montage "${args20[@]}" -tile 10x -geometry 20x20+6+6 $OUT/$2-20.png && convert $OUT/$2-20.png -filter point -resize 300% $OUT/$2-20.png
