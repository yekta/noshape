#!/bin/bash
# Runs on the perf / land server (vibe3, 10-30): one timed test command on a pushed commit, with vibe3 to itself, so the
# numbers are stable (this machine's load made perf numbers useless). Builds release first (not timed), then runs the
# command RUNS times; each run holds both land slots, so no land check (or other perf run) shares the machine while it
# runs; lands wait at most one run. Prints each run's wall time and the test's own timing lines; appends to
# ~/perf/history.tsv. perf.sh runs it from the commit itself:
#   git show FETCH_HEAD:tools/tiers/perf-remote.sh > ~/perf/remote-<name>.sh; bash ~/perf/remote-<name>.sh <name> <branch|sha> <runs> <cargo test args…>
set -uo pipefail
name=$1; ref=$2; runs=$3; shift 3
. "$HOME/noshape-env.sh"
mkdir -p "$HOME/perf" "$HOME/land"
# One perf run at a time, from the checkout on: a run waiting here had its tree replaced by the next one's checkout
# and timed that commit under its own sha (10-30).
exec 7>"$HOME/perf/lock"; flock 7
wt="$HOME/perf/wt"; [ -d "$wt" ] || git -C "$HOME/projects/noshape" worktree add -q --detach "$wt" origin/main
cd "$wt" && git fetch -q origin "$ref" 2>/dev/null; git checkout -q -f --detach FETCH_HEAD 2>/dev/null || git checkout -q -f --detach "$ref" || { echo "PERF FAILED: checkout"; exit 1; }
git clean -fdq
export CARGO_TARGET_DIR="$HOME/target-perf" NOSHAPE_CONFIG_DIR=$(mktemp -d -t noshape-config-XXXXXX)
sha=$(git rev-parse --short HEAD)
echo "== perf on $(hostname -s) at $sha: cargo test --release $*"
# Build without timing, outside the exclusive slots (lands go on meanwhile).
cargo test --release --locked --no-run "$@" > /tmp/perf-build-$$.log 2>&1 || { tail -20 /tmp/perf-build-$$.log; echo "PERF FAILED: build"; exit 1; }
times=()
for i in $(seq "$runs"); do
  exec 8>"$HOME/land/slot1"; flock 8; exec 9>"$HOME/land/slot2"; flock 9   # the machine to ourselves
  t0=$(date +%s.%N)
  out=$(cargo test --release --locked "$@" 2>&1); rc=$?
  wall=$(echo "$(date +%s.%N) - $t0" | bc)
  exec 8>&- 9>&-
  # The measure is the tests' own run time (libtest's "finished in", summed over test binaries), not cargo's wall
  # time: a rebuild cargo decides on between the build and the run (an OCCT library synced in meanwhile) once added
  # 395 s to a 161 s knurl (10-30).
  t=$(echo "$out" | sed -n 's/.*finished in \([0-9.]*\)s.*/\1/p' | awk '{s+=$1} END {printf "%.2f", s}')
  [ "$t" = "0.00" ] && t=$wall
  times+=("$(printf %.2f "$t")")
  echo "-- run $i: ${times[-1]} s in the tests ($(printf %.1f "$wall") s wall, exit $rc)"
  echo "$out" | grep -vE "^\s*(Compiling|Finished|Running|Blocking|Doc-tests)|^running [0-9]+ test|^$|^warning: " | grep -E "timed|×[0-9]|[0-9.]+ ?(ms|s)\b|mm³|faces|panicked|FAILED|test result" | head -25
  [ $rc -ne 0 ] && { echo "PERF FAILED: tests"; break; }
done
med=$(printf '%s\n' "${times[@]}" | sort -n | awk '{a[NR]=$1} END {print a[int((NR+1)/2)]}')
best=$(printf '%s\n' "${times[@]}" | sort -n | head -1)
echo "== $name $sha: best $best s, median $med s over ${#times[@]} run(s)"
printf '%s\t%s\t%s\t%s\t%s\t%s\n' "$(date -u '+%F %T')" "$name" "$sha" "$best" "$med" "$*" >> "$HOME/perf/history.tsv"
