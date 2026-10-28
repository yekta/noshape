#!/bin/bash
# Runs on the land server (vibe3, 10-28): land.sh's checks for a commit pushed to a temporary land/ branch, in the
# agent's own worktree and target dir there, at most two at a time. land.sh runs this script from the commit itself
# (git show FETCH_HEAD:tools/tiers/land-remote.sh > ~/land/remote-<name>.sh), the changed paths on stdin.
set -uo pipefail
name=$1; branch=$2; pkgs=$3
. "$HOME/noshape-env.sh"
mkdir -p "$HOME/land"; changed="$HOME/land/$name.changed"; cat > "$changed"
wt="$HOME/land/wt-$name"
[ -d "$wt" ] || git -C "$HOME/projects/noshape" worktree add -q --detach "$wt" FETCH_HEAD
cd "$wt" && git fetch -q origin "$branch" && git checkout -q -f --detach FETCH_HEAD && git clean -fdq || { echo "LAND FAILED: checkout on $(hostname -s)"; exit 1; }
export CARGO_TARGET_DIR="$HOME/target-land-$name"
echo "== on $(hostname -s) ($wt)"
# Two slots: the third concurrent land waits for one.
waited=0
while true; do
  for s in 1 2; do
    exec 9>"$HOME/land/slot$s"
    if flock -n 9; then
      [ $waited -gt 0 ] && echo "== waited $((waited * 5)) s for a slot"
      tools/tiers/land-checks.sh "$pkgs" "$changed"; exit $?
    fi
    exec 9>&-
  done
  waited=$((waited + 1)); sleep 5
done
