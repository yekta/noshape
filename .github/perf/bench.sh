#!/usr/bin/env bash
# Viewport benchmarks of a noshape build on a CI runner (macOS or Linux/Xvfb).
#
#   bench.sh <noshape binary> <document> <output dir> <modes...>
#
# For every presentation mode (`texture`, `readback`) and bench kind (orbit,
# zoom) the app opens a fresh copy of the document with NOSHAPE_PERF=1 and the
# built-in benchmark (viewport.rs `OrbitBench`), which logs a `BENCH SUMMARY`
# line. Screenshots: `<mode>-still.png` of the settled view before the orbit
# starts (the correctness check compares these across modes) and two taken
# during the orbit. Env: FRAMES (default 300), DELAY (seconds from launch
# until the bench starts, default 90), VARIANTS (extra texture-mode orbit runs
# for diagnosis, `name:VAR=value[,VAR=value]` separated by spaces, e.g.
# `msaa1:NOSHAPE_PERF_MSAA=1 nolines:NOSHAPE_PERF_SKIP=lines`; logged as
# `texture+<name>-orbit.log`).
set -u
BIN=$1; DOC=$2; OUT=$3; shift 3
MODES=("$@")
FRAMES=${FRAMES:-300}
DELAY=${DELAY:-90}
mkdir -p "$OUT"
shot() {
  if [[ "$(uname)" == Darwin ]]; then screencapture -x "$1"; else import -window root "$1"; fi
}
wait_for() { # pattern log seconds
  for _ in $(seq "$3"); do grep -qa "$1" "$2" && return 0; sleep 1; done
  return 1
}
RUNS=()
for mode in "${MODES[@]}"; do RUNS+=("$mode:orbit:" "$mode:zoom:"); done
for v in ${VARIANTS:-}; do RUNS+=("texture+${v%%:*}:orbit:${v#*:}"); done
for run in "${RUNS[@]}"; do
  label=${run%%:*}; rest=${run#*:}; kind=${rest%%:*}; extra=${rest#*:}
  mode=${label%%+*}
  {
    log="$OUT/$label-$kind.log"
    work=$(mktemp -d)
    cp "$DOC" "$work/bench.noshape"
    echo "== $label $kind ${extra//,/ }"
    env NOSHAPE_PERF=1 NOSHAPE_PERF_ORBIT="$FRAMES" NOSHAPE_PERF_ORBIT_DELAY="$DELAY" \
      NOSHAPE_PERF_BENCH="$kind" NOSHAPE_PRESENT="$mode" RUST_LOG=info ${extra//,/ } \
      "$BIN" "$work/bench.noshape" > "$log" 2>&1 &
    pid=$!
    if [[ $kind == orbit && $label == "$mode" ]]; then
      # The settled view (geometry refined, before the bench moves the camera).
      wait_for 'regen\[refine\]' "$log" $((DELAY - 15)) || echo "no refine regen logged"
      sleep 8
      shot "$OUT/$mode-still.png"
      if wait_for 'orbit bench: start' "$log" $((DELAY + 60)); then
        sleep 1.5; shot "$OUT/$mode-orbit-1.png"
        sleep 1.5; shot "$OUT/$mode-orbit-2.png"
      fi
    fi
    wait_for 'BENCH SUMMARY' "$log" $((DELAY + 600)) || echo "::warning::no BENCH SUMMARY for $mode $kind"
    kill "$pid" 2>/dev/null; sleep 2; kill -9 "$pid" 2>/dev/null
    grep -a "viewport frames\|GPU:\|orbit bench\|BENCH SUMMARY\|regen\[\|panicked\|ERROR" "$log" | cut -c1-400
    rm -rf "$work"
  }
done
