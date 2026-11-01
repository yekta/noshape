#!/bin/bash
# Runs on the land server (vibe4, else vibe3): land.sh's checks for a commit pushed to a temporary land/ branch, at
# most LAND_SLOTS at a time. land.sh runs this script from the commit itself
# (git show FETCH_HEAD:tools/tiers/land-remote.sh > ~/land/remote-<name>.sh), the changed paths on stdin.
# Each slot has its own checkout (~/land/wt-slot<N>) and target dir (~/target-land-slot<N>), reused by whichever land
# takes it: lands are all close to main, so they stay warm, and the disk holds LAND_SLOTS target dirs, not one per
# agent worktree (11-01: seven per-worktree dirs took 225 GB of vibe4's 400 in two hours).
set -uo pipefail
name=$1; branch=$2; pkgs=$3
. "$HOME/noshape-env.sh"
repo="$HOME/projects/noshape"
mkdir -p "$HOME/land"; changed="$HOME/land/$name.changed"; cat > "$changed"
git -C "$repo" fetch -q origin "+$branch:refs/land/$name" || { echo "LAND FAILED: fetch on $(hostname -s)"; exit 1; }
# Slots: one per ~5 cores (vibe3 8: 2, vibe4 16: 3), the cores shared out between them.
export LAND_SLOTS=$(( $(nproc) / 5 > 2 ? $(nproc) / 5 : 2 ))
export CARGO_BUILD_JOBS=$(( $(nproc) / LAND_SLOTS + 1 ))
# A slot, in arrival order (tools/tiers/slots.sh, taken from the commit being landed).
. <(git -C "$repo" show "refs/land/$name:tools/tiers/slots.sh")
take_slots 1
[ "$SLOT_WAITED" -gt 0 ] && echo "== waited $SLOT_WAITED s for a slot"
slot=${SLOT_IDS[0]}
wt="$HOME/land/wt-slot$slot"
[ -d "$wt" ] || git -C "$repo" worktree add -q --detach "$wt" "refs/land/$name"
cd "$wt" && git checkout -q -f --detach "refs/land/$name" && git clean -fdq || { echo "LAND FAILED: checkout on $(hostname -s)"; exit 1; }
export CARGO_TARGET_DIR="$HOME/target-land-slot$slot"
echo "== on $(hostname -s), slot $slot ($wt)"
# Disk: this target dir's old builds and stale incremental sessions go before each land.
python3 tools/tiers/prune-target.py "$CARGO_TARGET_DIR" > /dev/null 2>&1
for d in "$CARGO_TARGET_DIR"/debug/incremental/*/; do
  [ -n "$(find "$d" -maxdepth 1 -name '*-working' -print -quit 2>/dev/null)" ] && continue
  [ -n "$(find "$d" -maxdepth 0 -mmin +120 -print 2>/dev/null)" ] && rm -rf "$d"
done
# Below 60 GB free, other idle land target dirs go, least recently used first (older per-worktree ones included).
free_gb() { df -BG --output=avail / | tail -1 | tr -dc 0-9; }
for t in $(ls -dtr "$HOME"/target-land-* 2>/dev/null); do
  [ "$(free_gb)" -ge 60 ] && break
  [ "$t" = "$CARGO_TARGET_DIR" ] && continue
  busy=0; for p in $(pgrep -x cargo); do tr '\0' '\n' < /proc/$p/environ 2>/dev/null | grep -qx "CARGO_TARGET_DIR=$t" && busy=1; done
  [ $busy = 0 ] && rm -rf "$t" && echo "== removed $t (idle; disk below 60 GB free)"
done
tools/tiers/land-checks.sh "$pkgs" "$changed"; rc=$?
git -C "$repo" update-ref -d "refs/land/$name" 2>/dev/null
exit $rc
