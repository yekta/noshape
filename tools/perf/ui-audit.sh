#!/usr/bin/env bash
# The UI responsiveness audit (docs/parity/interaction-feel.md "UI responsiveness audit"): the scenarios of
# tests/perf/ui-audit run in the app through the ui-parity driver's timing mode, `rounds` times, then each step's
# medians: its handler, the UI thread's CPU to its first frame, that frame's end and the window's idle (ms). Fails when
# a median first frame is over 100 ms or its UI-thread CPU over 50 ms.
#   tools/perf/ui-audit.sh <noshape binary (an optimised build: --profile ui-bench)> <X display> [rounds] [out dir]
# Run from the repository's root (the Part Studio scenario opens tests/perf/live-30-features.noshape).
set -euo pipefail
app=$1 display=$2 rounds=${3:-3} out=${4:-$(mktemp -d)}
for k in $(seq 1 "$rounds"); do
  for s in audit-part-studio audit-assembly; do
    NOSHAPE_UI_TIMING=1 NOSHAPE_PERF=1 python3 tools/ui-parity/run.py --app "$app" --scenarios tests/perf/ui-audit \
      --out "$out/round$k" --display "$display" --timeout 900 --all "$s" | tail -1
  done
done
python3 tools/perf/ui-audit-medians.py "$out" audit-part-studio audit-assembly
