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
# Two lands at once on 8 cores: 4 compile jobs each.
export CARGO_BUILD_JOBS=4
echo "== on $(hostname -s) ($wt)"
# Disk (10-29: six target dirs reached 230 GB in a night): before each land, this target dir's old builds and stale
# incremental sessions go; below 60 GB free, the least recently used idle land target dir goes whole (rebuilt on its next land).
python3 tools/tiers/prune-target.py "$CARGO_TARGET_DIR" > /dev/null 2>&1
for d in "$CARGO_TARGET_DIR"/debug/incremental/*/; do
  [ -n "$(find "$d" -maxdepth 1 -name '*-working' -print -quit 2>/dev/null)" ] && continue
  [ -n "$(find "$d" -maxdepth 0 -mmin +120 -print 2>/dev/null)" ] && rm -rf "$d"
done
free_gb() { df -BG --output=avail / | tail -1 | tr -dc 0-9; }
for t in $(ls -dtr "$HOME"/target-land-* 2>/dev/null); do
  [ "$(free_gb)" -ge 60 ] && break
  [ "$t" = "$CARGO_TARGET_DIR" ] && continue
  busy=0; for p in $(pgrep -x cargo); do tr '\0' '\n' < /proc/$p/environ 2>/dev/null | grep -qx "CARGO_TARGET_DIR=$t" && busy=1; done
  [ $busy = 0 ] && rm -rf "$t" && echo "== removed $t (idle; disk below 60 GB free)"
done
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
