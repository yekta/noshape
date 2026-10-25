#!/usr/bin/env bash
# Navigation replay on Linux with real kernel input devices (needs root for
# /dev/uinput): a headless sway reads virtual uinput devices through libinput
# exactly like a laptop's mouse, touchpad and touchscreen, and noshape runs
# under it on Wayland (`wayland`) or through Xwayland on X11 (`x11`).
#
#   linux-replay.sh <noshape binary> <document> <output dir> <wayland|x11>
#
# replay.py drives the devices; the app writes every input event and camera
# (NOSHAPE_NAV_TRACE) and its Wayland protocol traffic (WAYLAND_DEBUG); then
# check.py recomputes from the protocol events what Chrome + Onshape would do
# (navigation.md "Chrome's input mapping") and compares it with the camera.
set -u
BIN=$1; DOC=$2; OUT=$3; MODE=${4:-wayland}
HERE=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$OUT"
OUT=$(cd "$OUT" && pwd)
export XDG_RUNTIME_DIR="$OUT/xdg"
mkdir -p "$XDG_RUNTIME_DIR"; chmod 700 "$XDG_RUNTIME_DIR"
cat > "$OUT/sway.conf" <<'EOF'
output HEADLESS-1 resolution 1600x1000 position 0 0 scale 1
input type:pointer {
  accel_profile flat
  pointer_accel 0
}
input type:touchpad {
  tap disabled
  natural_scroll disabled
  accel_profile flat
  pointer_accel 0
  dwt disabled
}
input type:touch map_to_output HEADLESS-1
input type:tablet_tool map_to_output HEADLESS-1
default_border none
default_floating_border none
gaps inner 0
xwayland enable
EOF
# The noop seat opens the devices directly (root) without taking a VT.
env WLR_BACKENDS=headless,libinput WLR_LIBINPUT_NO_DEVICES=1 WLR_RENDERER=pixman \
  WLR_HEADLESS_OUTPUTS=1 LIBSEAT_BACKEND=noop \
  sway -c "$OUT/sway.conf" > "$OUT/sway.log" 2>&1 &
SWAY=$!
for _ in $(seq 30); do ls "$XDG_RUNTIME_DIR"/wayland-* >/dev/null 2>&1 && break; sleep 0.5; done
export WAYLAND_DISPLAY=$(basename "$(ls "$XDG_RUNTIME_DIR"/wayland-? | head -1)")
export SWAYSOCK=$(ls "$XDG_RUNTIME_DIR"/sway-ipc.* | head -1)
work=$(mktemp -d)
cp "$DOC" "$work/nav.noshape"
TRACE="$OUT/trace-$MODE.jsonl"
rm -f "$TRACE"
if [[ $MODE == x11 ]]; then
  # Sway's children get its Xwayland display; ask one for it.
  rm -f "$OUT/display"
  swaymsg exec "echo \$DISPLAY > $OUT/display" > /dev/null
  for _ in $(seq 20); do [[ -s $OUT/display ]] && break; sleep 0.5; done
  DISPLAY=$(cat "$OUT/display" 2>/dev/null)
  [[ -n $DISPLAY ]] || { echo "no Xwayland display"; kill "$SWAY"; exit 1; }
  export DISPLAY
  env -u WAYLAND_DISPLAY NOSHAPE_NAV_TRACE="$TRACE" RUST_LOG=info "$BIN" "$work/nav.noshape" > "$OUT/app-$MODE.log" 2>&1 &
else
  env -u DISPLAY NOSHAPE_NAV_TRACE="$TRACE" WAYLAND_DEBUG=client RUST_LOG=info "$BIN" "$work/nav.noshape" > "$OUT/app-$MODE.log" 2>&1 &
fi
APP=$!
python3 -I "$HERE/replay.py" --trace "$TRACE" --mode "$MODE" --out "$OUT"
status=$?
kill "$APP" 2>/dev/null; sleep 1; kill -9 "$APP" 2>/dev/null
kill "$SWAY" 2>/dev/null; sleep 1; kill -9 "$SWAY" 2>/dev/null
rm -rf "$work"
[[ $status == 0 ]] || exit $status
python3 -I "$HERE/check.py" --trace "$TRACE" --wayland-log "$OUT/app-$MODE.log" --mode "$MODE" | tee "$OUT/check-$MODE.txt"
exit "${PIPESTATUS[0]}"
