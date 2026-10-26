#!/usr/bin/env bash
# Builds a static, headless OpenCascade (OCCT) for noshape.
# Usage: scripts/build-occt.sh [install-prefix]
# Result: <prefix>/include/opencascade/*.hxx and <prefix>/lib/*.a
# Then build noshape with OCCT_ROOT=<prefix>.
set -euo pipefail
OCCT_TAG="${OCCT_TAG:-V8_0_1}"
PREFIX="${1:-$HOME/.cache/noshape/occt-${OCCT_TAG}}"
WORK="${OCCT_WORK:-$HOME/.cache/noshape/occt-build-${OCCT_TAG}}"
JOBS="${JOBS:-$(getconf _NPROCESSORS_ONLN 2>/dev/null || sysctl -n hw.ncpu)}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
mkdir -p "$WORK"
cd "$WORK"
SRC="OCCT-${OCCT_TAG#V}"
# noshape's patches to OCCT (patches/occt/*.patch, applied in name order to a
# fresh source tree; the marker re-extracts it when the patch set changes).
PATCHES="$ROOT/patches/occt"
PATCH_SET="$(cat "$PATCHES"/*.patch 2>/dev/null | cksum | cut -d' ' -f1)"
if [ -d "$SRC" ] && [ "$(cat "$SRC/.noshape-patches" 2>/dev/null)" != "$PATCH_SET" ]; then
  rm -rf "$SRC" build
fi
if [ ! -d "$SRC" ]; then
  curl -sSL -o occt.tgz "https://github.com/Open-Cascade-SAS/OCCT/archive/refs/tags/${OCCT_TAG}.tar.gz"
  tar xzf occt.tgz && rm occt.tgz
  for p in "$PATCHES"/*.patch; do
    [ -e "$p" ] || continue
    echo "Applying $(basename "$p")"
    patch -p1 -d "$SRC" < "$p"
  done
  echo "$PATCH_SET" > "$SRC/.noshape-patches"
fi
GEN=()
command -v ninja >/dev/null && GEN=(-G Ninja)
cmake -S "$SRC" -B build "${GEN[@]}" \
  -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_INSTALL_PREFIX="$PREFIX" \
  -DCMAKE_POSITION_INDEPENDENT_CODE=ON \
  -DBUILD_LIBRARY_TYPE=Static \
  -DBUILD_DOC_Overview=OFF \
  -DBUILD_MODULE_Draw=OFF \
  -DBUILD_MODULE_FoundationClasses=ON \
  -DBUILD_MODULE_ModelingData=ON \
  -DBUILD_MODULE_ModelingAlgorithms=ON \
  -DBUILD_MODULE_Visualization=ON \
  -DBUILD_MODULE_ApplicationFramework=ON \
  -DBUILD_MODULE_DataExchange=ON \
  -DBUILD_ADDITIONAL_TOOLKITS="" \
  -DUSE_OPENGL=OFF -DUSE_GLES2=OFF -DUSE_D3D=OFF \
  -DUSE_FREETYPE=OFF -DUSE_FREEIMAGE=OFF -DUSE_OPENVR=OFF \
  -DUSE_TK=OFF -DUSE_TCL=OFF -DUSE_VTK=OFF -DUSE_FFMPEG=OFF \
  -DUSE_RAPIDJSON=OFF -DUSE_DRACO=OFF -DUSE_TBB=OFF \
  -DBUILD_SAMPLES_QT=OFF -DBUILD_Inspector=OFF \
  -DINSTALL_DIR_LAYOUT=Unix
cmake --build build --parallel "$JOBS"
cmake --install build
echo "OCCT installed to $PREFIX"
