#!/bin/bash
# The viewport's shaders compiled ahead of time into <app>/Contents/Resources/metallib
# (vendor/wgpu-hal/src/metal/precompiled.rs), so a first launch loads them instead
# of compiling them (~0.5 s). Run from the repository root after the release build.
set -euo pipefail
app=$1
msl=$(mktemp -d)
out="$app/Contents/Resources/metallib"
NOSHAPE_MSL_DUMP="$msl" cargo run --release --locked -p noshape-render --example prewarm_shaders
mkdir -p "$out"
for f in "$msl"/*.metal; do
  k=$(basename "$f" .metal)
  # naga's first line: "// language: metal3.1"
  std=$(head -1 "$f" | sed -n 's|^// language: metal\([0-9.]*\).*|\1|p')
  xcrun -sdk macosx metal -std="metal${std:-3.0}" -fpreserve-invariance -c "$f" -o "$msl/$k.air"
  xcrun -sdk macosx metallib "$msl/$k.air" -o "$out/$k.metallib"
done
# Every pipeline must now load its library (the key is stable).
log=$(NOSHAPE_METALLIB_DIR="$out" NOSHAPE_METALLIB_LOG=1 cargo run --release --locked -p noshape-render --example prewarm_shaders 2>&1 | grep '^metallib ' || true)
echo "$log"
echo "metallibs: $(ls "$out" | wc -l) built, $(grep -c precompiled <<<"$log" || true) loaded, $(grep -c 'compiled now' <<<"$log" || true) compiled"
if grep -q 'compiled now' <<<"$log"; then
  echo "a pipeline compiled its shader although its metallib was built" >&2
  exit 1
fi
