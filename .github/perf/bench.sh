#!/usr/bin/env bash
# Viewport benchmarks of a noshape build on a CI runner (macOS or Linux/Xvfb).
#
#   bench.sh <noshape binary> <document> <output dir> <modes...>
#
# For every presentation mode (`texture`, `readback`) and bench kind (orbit,
# zoom, pan; `BENCH_ENV` NOSHAPE_PERF_BENCH=drag makes every run a tool drag
# of what is under the viewport centre, for assemblies) the app opens a fresh
# copy of the document with NOSHAPE_PERF=1 and the
# built-in benchmark (viewport.rs `OrbitBench`), which logs a `BENCH SUMMARY`
# line. Screenshots: `<mode>-still.png` of the settled view before the orbit
# starts (the correctness check compares these across modes) and two taken
# during the orbit. Env: FRAMES (default 300), DELAY (seconds from launch
# until the bench starts, default 90), VARIANTS (extra texture-mode orbit runs
# for diagnosis, `name:VAR=value[,VAR=value]` separated by spaces, e.g.
# `msaa1:NOSHAPE_PERF_MSAA=1 nolines:NOSHAPE_PERF_SKIP=lines`; `name@pan:…` runs a pan (or zoom) instead;
# logged as `texture+<name>-<kind>.log`), BENCH_ENV (`VAR=value[,VAR=value]` for every
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
# ONLY (`<label>:<kind>` separated by spaces) runs just those.
# OPEN_DOCS (documents separated by spaces): each opened 3 times (`NOSHAPE_PERF_SCRIPT=open`, which quits once the
# full model is on screen), one `BENCH OPEN <doc> model=<ms> full=<ms>` line per run in open.log: "opening document"
# to the first frame drawing the model (its preview display) and the refined one (PLAN 2.7 ≤ 0.5 s), with AB_BIN too.
# LATENCY_DOC: also `NOSHAPE_PERF_SCRIPT=ui-latency` on that Part Studio (feature dialogs opened and list rows
# clicked, the UI thread's part of showing each: `latency.log`), with AB_BIN too.
# AB_BIN / AB_OUT: every run (but the pick bench) also with another build (the previous release, the same-job
# baseline perf.yml compares with), alternating which goes first.
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
# BASE=0: only the variants (the big-model rows of the 60 fps floor: their 2× runs).
if [[ ${BASE:-1} == 1 ]]; then
  for mode in "${MODES[@]}"; do RUNS+=("$mode:orbit:" "$mode:zoom:" "$mode:pan:"); done
fi
# Selection latency (`NOSHAPE_PERF_BENCH=pick`: clicks across the model, click to the frame with its selection), once.
[[ ${PICK:-1} == 1 && ${BASE:-1} == 1 ]] && RUNS+=("${MODES[0]}:pick:")
# A variant `name@kind:VARS` runs that bench kind (orbit, pan or zoom; default orbit).
for v in ${VARIANTS:-}; do
  name=${v%%:*}; vkind=orbit
  if [[ $name == *@* ]]; then vkind=${name#*@}; name=${name%%@*}; fi
  RUNS+=("texture+$name:$vkind:${v#*:}")
done
# One run of `bin` into `out` (`shots`: the correctness stills of the mode's orbit).
one() {
  local bin=$1 out=$2 shots=$3
  mkdir -p "$out"
  log="$out/$label-$kind.log"
  work=$(mktemp -d)
  cp "$DOC" "$work/bench.noshape"
  echo "== $label $kind ${extra//,/ } ($bin)"
  if [[ -n ${COLD:-} && "$(uname)" == Darwin ]]; then
    cache=$(getconf DARWIN_USER_CACHE_DIR)
    echo "cold start: clearing $(du -sk "$cache" 2>/dev/null | cut -f1) KB cache, Metal: $(find "$cache" -maxdepth 3 -type d -name com.apple.metal | tr '\n' ' ')"
    find "$cache" -maxdepth 3 -type d -name com.apple.metal -prune -exec rm -rf {} +
  fi
  env NOSHAPE_PERF=1 NOSHAPE_PERF_ORBIT="$FRAMES" NOSHAPE_PERF_ORBIT_DELAY="$DELAY" \
    NOSHAPE_PERF_BENCH="$kind" NOSHAPE_PRESENT="$mode" RUST_LOG=info ${BENCH_ENV//,/ } ${extra//,/ } \
    "$bin" "$work/bench.noshape" > "$log" 2>&1 &
  pid=$!
  if [[ $shots == 1 && $kind == orbit && $label == "$mode" ]]; then
    # The settled view (geometry refined, before the bench moves the camera).
    wait_for 'regen\[refine\]' "$log" $((DELAY - 15)) || echo "no refine regen logged"
    sleep 8
    shot "$out/$mode-still.png"
    if wait_for 'orbit bench: start' "$log" $((DELAY + 60)); then
      sleep 1.5; shot "$out/$mode-orbit-1.png"
      sleep 1.5; shot "$out/$mode-orbit-2.png"
    fi
  fi
  wait_for 'BENCH SUMMARY' "$log" $((DELAY + 600)) || echo "::warning::no BENCH SUMMARY for $mode $kind"
  kill "$pid" 2>/dev/null; sleep 2; kill -9 "$pid" 2>/dev/null
  grep -a "viewport frames\|GPU:\|orbit bench\|BENCH FIRST\|BENCH SUMMARY\|regen\[\|panicked\|ERROR" "$log" | cut -c1-400
  rm -rf "$work"
}
n=0
for run in "${RUNS[@]}"; do
  label=${run%%:*}; rest=${run#*:}; kind=${rest%%:*}; extra=${rest#*:}
  # ONLY="<label>:<kind> ...": just those runs (perf.yml re-measures the rows a baseline comparison flagged).
  [[ -n ${ONLY:-} && " $ONLY " != *" $label:$kind "* ]] && continue
  mode=${label%%+*}
  # AB_BIN: the same run of that build too, into AB_OUT, right before or after (alternately, so the runner slowing
  # down through the job weighs on both alike); the pick bench only for the build measured.
  if [[ -n ${AB_BIN:-} && $kind != pick ]]; then
    n=$((n + 1))
    if (( n % 2 )); then one "$BIN" "$OUT" 1; one "$AB_BIN" "$AB_OUT" 0; else one "$AB_BIN" "$AB_OUT" 0; one "$BIN" "$OUT" 1; fi
  else
    one "$BIN" "$OUT" 1
  fi
done
# The UI latency of a feature dialog's opening and a list row's click (perf_script.rs), once.
latency() {
  local bin=$1 out=$2
  mkdir -p "$out"
  local log="$out/latency.log" work
  work=$(mktemp -d)
  cp "$LATENCY_DOC" "$work/latency.noshape"
  echo "== latency $(basename "$LATENCY_DOC") ($bin)"
  env NOSHAPE_PERF=1 NOSHAPE_PERF_SCRIPT=ui-latency NOSHAPE_PRESENT="${MODES[0]}" RUST_LOG=info ${BENCH_ENV//,/ } \
    "$bin" "$work/latency.noshape" > "$log" 2>&1 &
  local pid=$!
  wait_for 'BENCH SUMMARY row-click' "$log" $((DELAY + 300)) || echo "::warning::no latency BENCH SUMMARY"
  kill "$pid" 2>/dev/null; sleep 2; kill -9 "$pid" 2>/dev/null
  grep -a "BENCH SUMMARY\|panicked\|ERROR" "$log" | cut -c1-400
  rm -rf "$work"
}
# Document open → its model on screen (perf_script.rs `open`).
open_times() {
  local bin=$1 out=$2
  mkdir -p "$out"
  local log="$out/open.log"
  for doc in $OPEN_DOCS; do
    for run in 1 2 3; do
      local work run_log
      work=$(mktemp -d)
      for ext in "" -wal -shm; do [[ -f "$doc$ext" ]] && cp "$doc$ext" "$work/open.noshape$ext"; done
      run_log="$work/run.log"
      env NOSHAPE_PERF=1 NOSHAPE_PERF_SCRIPT=open NOSHAPE_PRESENT="${MODES[0]}" RUST_LOG=info ${BENCH_ENV//,/ } \
        "$bin" "$work/open.noshape" > "$run_log" 2>&1 &
      local pid=$!
      for _ in $(seq 180); do kill -0 "$pid" 2>/dev/null || break; sleep 1; done
      kill "$pid" 2>/dev/null; sleep 1; kill -9 "$pid" 2>/dev/null
      awk -v doc="$(basename "$doc" .noshape)" '
        /opening document: / { for (i = 1; i <= NF; i++) if ($i == "document:") o = $(i + 1) }
        /open: preview model frame presented: / { for (i = 1; i <= NF; i++) if ($i == "presented:") p = $(i + 1) }
        /open: full model frame presented: / { for (i = 1; i <= NF; i++) if ($i == "presented:") f = $(i + 1) }
        END { if (p == "") p = f; printf "BENCH OPEN %s model=%s full=%s\n", doc, (p == "" ? "-" : p - o), (f == "" ? "-" : f - o) }
      ' "$run_log" | tee -a "$log"
      rm -rf "$work"
    done
  done
}
if [[ -n ${OPEN_DOCS:-} && ( -z ${ONLY:-} || " $ONLY " == *" open:"* ) ]]; then
  open_times "$BIN" "$OUT"
  # (A baseline before the `open` script would wait out every run.)
  if [[ -n ${AB_BIN:-} ]] && grep -aq "open: preview model frame presented" "$AB_BIN"; then open_times "$AB_BIN" "$AB_OUT"; fi
fi
if [[ -n ${LATENCY_DOC:-} && ( -z ${ONLY:-} || " $ONLY " == *" latency:"* ) ]]; then
  # The build run first pays the colder caches (perf 42411762630, macOS 26: its first click 4-5 ms more, twice): the
  # re-measure of a flagged row (ONLY) runs the baseline first, so a regression has to show in both orders.
  if [[ -n ${AB_BIN:-} && -n ${ONLY:-} ]]; then
    latency "$AB_BIN" "$AB_OUT"
    latency "$BIN" "$OUT"
  else
    latency "$BIN" "$OUT"
    [[ -n ${AB_BIN:-} ]] && latency "$AB_BIN" "$AB_OUT"
  fi
fi
