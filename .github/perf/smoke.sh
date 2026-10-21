#!/usr/bin/env bash
# Release smoke test (perf.yml job `smoke`, crates/app/src/smoke.rs):
# smoke.sh <noshape binary> <output dir>
# Runs the app with NOSHAPE_SMOKE=1 on the demo document; fails on a crash,
# an abort, a hang (killed after LIMIT seconds), a non-zero exit or a run
# that never logged "SMOKE done". macOS crash reports written during the run
# are copied into the output dir.
set -uo pipefail
bin="$1"
out="$2"
LIMIT="${LIMIT:-480}"
mkdir -p "$out"
log="$out/smoke.log"
start="$out/.start"
touch "$start"

[[ "$(uname)" == Darwin ]] && sw_vers
NOSHAPE_SMOKE=1 "$bin" --demo-drawing > "$log" 2>&1 &
pid=$!
( sleep "$LIMIT"; kill -9 "$pid" 2>/dev/null && echo "SMOKE KILLED: no exit after ${LIMIT}s" >> "$log" ) &
watchdog=$!
wait "$pid"
status=$?
kill "$watchdog" 2>/dev/null
wait "$watchdog" 2>/dev/null

if [[ "$(uname)" == Darwin ]]; then
  # ReportCrash writes the report a few seconds after the process dies.
  [[ $status == 0 ]] || sleep 15
  for d in ~/Library/Logs/DiagnosticReports /Library/Logs/DiagnosticReports; do
    find "$d" -newer "$start" -type f \( -iname '*noshape*' -o -iname '*.ips' -o -iname '*.crash' \) \
      -exec cp {} "$out/" \; 2>/dev/null
  done
fi
rm -f "$start"

grep -E "^SMOKE" "$log"
echo "--- last lines of the log"
tail -n 40 "$log"
echo "exit status $status"
ls -la "$out"
if [[ $status != 0 ]]; then
  echo "::error::noshape exited with status $status (smoke test)"
  exit 1
fi
if ! grep -q "^SMOKE done" "$log"; then
  echo "::error::noshape exited without finishing the smoke test"
  exit 1
fi
