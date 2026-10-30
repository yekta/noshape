#!/bin/bash
# perf.sh: time a test on the perf server (vibe3) at this worktree's commit, with the machine to itself (stable numbers;
# this machine's load makes local timings useless). Commit first; the commit goes to a temporary perf/ branch.
# Usage: tools/tiers/perf.sh [-n RUNS] <cargo test args>, e.g.
#   tools/tiers/perf.sh -p noshape-model --test all -- custom_features::knurl_feature_b94_on_a_cylinder --ignored --nocapture --test-threads=1
# Prints each run's wall time, the test's timing lines, best and median; vibe3 keeps ~/perf/history.tsv.
set -uo pipefail
cd "$(git rev-parse --show-toplevel)"
runs=3; [ "${1:-}" = -n ] && { runs=$2; shift 2; }
[ $# -gt 0 ] || { sed -n 2,7p "$0"; exit 2; }
[ -z "$(git status --porcelain --untracked-files=no)" ] || echo "note: uncommitted changes are not measured (commit them first)"
name=$(basename "$PWD"); branch="perf/$name-$(git rev-parse --short HEAD)"
git push -q -f origin "HEAD:refs/heads/$branch" 2>/dev/null || { echo "PERF FAILED: push to $branch"; exit 1; }
args=$(printf '%q ' "$@")
ssh -o BatchMode=yes vibe3 "mkdir -p ~/perf && cd ~/projects/noshape && git fetch -q origin $branch && git show FETCH_HEAD:tools/tiers/perf-remote.sh > ~/perf/remote-$name.sh && bash ~/perf/remote-$name.sh '$name' '$branch' $runs $args"
rc=$?; git push -q origin --delete "$branch" 2>/dev/null; exit $rc
