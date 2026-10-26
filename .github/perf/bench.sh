#!/usr/bin/env bash
# Viewport benchmarks of a noshape build on a CI runner (macOS or Linux/Xvfb).
#
#   bench.sh <noshape binary> <document> <output dir> <modes...>
#
# For every presentation mode (`texture`, `readback`) and bench kind (orbit,
# zoom, pan) the app opens a fresh copy of the document with NOSHAPE_PERF=1 and the
# built-in benchmark (viewport.rs `OrbitBench`), which logs a `BENCH SUMMARY`
# line. Screenshots: `<mode>-still.png` of the settled view before the orbit
# starts (the correctness check compares these across modes) and two taken
# during the orbit. Env: FRAMES (default 300), DELAY (seconds from launch
# until the bench starts, default 90), VARIANTS (extra texture-mode orbit runs
# for diagnosis, `name:VAR=value[,VAR=value]` separated by spaces, e.g.
# `msaa1:NOSHAPE_PERF_MSAA=1 nolines:NOSHAPE_PERF_SKIP=lines`; logged as
# `texture+<name>-orbit.log`), BENCH_ENV (`VAR=value[,VAR=value]` for every
# run, before a variant's own), COLD (macOS: clear the Metal shader cache
# before every launch, so each launch is a cold start).
#
# The macOS runners: only the first run of a job (the texture orbit, about a
# minute in) is reliable for presented fps. A few minutes into a job the
# runner slows the app's window down (the orbit's 8 ms timer then fires every
# ~62 ms: parts-1000 at 33 fps in the first orbit, 19 fps later, for any
# build: v0.4.7 and v0.4.8 alike), so later runs and variants are only
# comparable with each other; compare settings for fps in separate jobs
# (BENCH_ENV), GPU time (`w.gpu_wait`) within a job.
set -u
BIN=$1; DOC=$2; OUT=$3; shift 3
MODES=("$@")
FRAMES=${FRAMES:-300}
DELAY=${DELAY:-90}
BENCH_ENV=${BENCH_ENV:-}
mkdir -p "$OUT"
shot() {
  if [[ "$(uname)" == Darwin ]]; then screencapture -x "$1"; else import -window root "$1"; fi
}
wait_for() { # pattern log seconds
  for _ in $(seq "$3"); do grep -qa "$1" "$2" && return 0; sleep 1; done
  return 1
}
RUNS=()
for mode in "${MODES[@]}"; do RUNS+=("$mode:orbit:" "$mode:zoom:" "$mode:pan:"); done
for v in ${VARIANTS:-}; do RUNS+=("texture+${v%%:*}:orbit:${v#*:}"); done
for run in "${RUNS[@]}"; do
  label=${run%%:*}; rest=${run#*:}; kind=${rest%%:*}; extra=${rest#*:}
  mode=${label%%+*}
  {
    log="$OUT/$label-$kind.log"
    work=$(mktemp -d)
    cp "$DOC" "$work/bench.noshape"
    echo "== $label $kind ${extra//,/ }"
    if [[ -n ${COLD:-} && "$(uname)" == Darwin ]]; then
      cache=$(getconf DARWIN_USER_CACHE_DIR)
      echo "cold start: clearing $(du -sk "$cache" 2>/dev/null | cut -f1) KB cache, Metal: $(find "$cache" -maxdepth 3 -type d -name com.apple.metal | tr '\n' ' ')"
      find "$cache" -maxdepth 3 -type d -name com.apple.metal -prune -exec rm -rf {} +
    fi
    env NOSHAPE_PERF=1 NOSHAPE_PERF_ORBIT="$FRAMES" NOSHAPE_PERF_ORBIT_DELAY="$DELAY" \
      NOSHAPE_PERF_BENCH="$kind" NOSHAPE_PRESENT="$mode" RUST_LOG=info ${BENCH_ENV//,/ } ${extra//,/ } \
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
