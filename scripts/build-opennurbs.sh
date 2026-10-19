#!/usr/bin/env bash
# Builds a static openNURBS (Rhino .3dm read/write toolkit, MIT-style license)
# for noshape's Rhino export.
# Usage: scripts/build-opennurbs.sh [install-prefix]
# Result: <prefix>/include/*.h (+ zlib/) and <prefix>/lib/*.a
# Then build noshape with OPENNURBS_ROOT=<prefix> (CI and releases always do).
set -euo pipefail
ON_REV="${OPENNURBS_REV:-eb92af3ba1806b0a34a99aba0d3bda83e3d46083}" # 8.x branch
PREFIX="${1:-$HOME/.cache/noshape/opennurbs-${ON_REV:0:12}}"
WORK="${OPENNURBS_WORK:-$HOME/.cache/noshape/opennurbs-build-${ON_REV:0:12}}"
JOBS="${JOBS:-$(getconf _NPROCESSORS_ONLN 2>/dev/null || sysctl -n hw.ncpu)}"
mkdir -p "$WORK"
cd "$WORK"
SRC="opennurbs-${ON_REV}"
if [ ! -d "$SRC" ]; then
  curl -sSL -o on.tgz "https://github.com/mcneel/opennurbs/archive/${ON_REV}.tar.gz"
  tar xzf on.tgz && rm on.tgz
fi
GEN=()
command -v ninja >/dev/null && GEN=(-G Ninja)
# CMAKE_POLICY_VERSION_MINIMUM: android_uuid's CMakeLists asks for an old CMake.
cmake -S "$SRC" -B build "${GEN[@]}" \
  -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_POSITION_INDEPENDENT_CODE=ON \
  -DCMAKE_POLICY_VERSION_MINIMUM=3.5
cmake --build build --config Release --parallel "$JOBS" --target opennurbsStatic
mkdir -p "$PREFIX/include/zlib" "$PREFIX/lib"
cp "$SRC"/*.h "$PREFIX/include/"
cp "$SRC"/zlib/*.h "$PREFIX/include/zlib/"
find build \( -name '*.a' -o -name '*.lib' \) -exec cp {} "$PREFIX/lib/" \;
ls "$PREFIX/lib"
