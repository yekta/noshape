# The land server's slots (~/land/slot1..slot$LAND_SLOTS, default 2), taken in arrival order. Sourced by land-daemon.sh and land-remote.sh (one slot)
# and perf-remote.sh (both: the machine to itself). Before 10-31 lands polled the slots every 5 s and perf runs
# waited in flock, so whoever looked at the right moment went first: one land waited 70 min while later ones went.
# Now each waiter appends itself to ~/land/queue and only the first live entry may take slots; it leaves the queue
# once it holds them, so the next one takes the other slot.
# take_slots 1 | all: returns holding one slot or every slot (numbers in SLOT_IDS, fds in SLOT_FDS; release_slots, or
# the script's exit, lets them go); SLOT_WAITED = seconds waited.
take_slots() {
  local want=$1 q="$HOME/land/queue" me first i fd n=${LAND_SLOTS:-2} held=()
  SLOT_FDS=()
  [ "$want" = all ] && want=$n
  # (BASHPID: the land daemon's workers are subshells; with $$ their entries died with a restarted daemon, 11-01.)
  me="$(date +%s%N) ${BASHPID:-$$}"; SLOT_WAITED=0
  mkdir -p "$HOME/land"; touch "$q"
  { flock 4; echo "$me" >> "$q"; } 4>"$q.lock"
  # Gone from the queue however this script ends (a killed waiter's entry is also dropped below: its pid is dead).
  trap '{ flock 4; grep -vxF "'"$me"'" "'"$q"'" > "'"$q"'.tmp"; mv "'"$q"'.tmp" "'"$q"'"; } 4>"'"$q"'.lock"' EXIT
  while true; do
    first=$({ flock 4; awk '{ if (system("kill -0 " $2 " 2>/dev/null") == 0) print }' "$q" > "$q.tmp"; mv "$q.tmp" "$q"; head -1 "$q"; } 4>"$q.lock")
    if [ "$first" = "$me" ]; then
      # (Slots taken in an earlier round are kept: perf waits holding some for the rest, ahead of later arrivals.)
      for i in $(seq "$n"); do
        [ ${#held[@]} -ge "$want" ] && break
        [[ " ${held[*]} " == *" $i "* ]] && continue
        exec {fd}>"$HOME/land/slot$i"; if flock -n $fd; then held+=("$i"); SLOT_FDS+=("$fd"); else exec {fd}>&-; fi
      done
      if [ ${#held[@]} -ge "$want" ]; then
        { flock 4; grep -vxF "$me" "$q" > "$q.tmp"; mv "$q.tmp" "$q"; } 4>"$q.lock"
        SLOT_IDS=("${held[@]}"); trap - EXIT; return 0
      fi
    fi
    sleep 5; SLOT_WAITED=$((SLOT_WAITED + 5))
  done
}
release_slots() { local fd; for fd in "${SLOT_FDS[@]}"; do exec {fd}>&-; done; SLOT_FDS=(); }
# Before a land uses a slot's target dir: its old builds and stale incremental sessions go; below 60 GB free, other idle
# land target dirs go, least recently used first (older per-worktree ones included). Run from the checkout.
prune_land_target() {
  local t=$1 d p busy
  python3 tools/tiers/prune-target.py "$t" > /dev/null 2>&1
  for d in "$t"/debug/incremental/*/; do
    [ -n "$(find "$d" -maxdepth 1 -name '*-working' -print -quit 2>/dev/null)" ] && continue
    [ -n "$(find "$d" -maxdepth 0 -mmin +120 -print 2>/dev/null)" ] && rm -rf "$d"
  done
  for d in $(ls -dtr "$HOME"/target-land-* 2>/dev/null); do
    [ "$(df -BG --output=avail / | tail -1 | tr -dc 0-9)" -ge 60 ] && break
    [ "$d" = "$t" ] && continue
    busy=0; for p in $(pgrep -x cargo); do tr '\0' '\n' < /proc/$p/environ 2>/dev/null | grep -qx "CARGO_TARGET_DIR=$d" && busy=1; done
    [ $busy = 0 ] && rm -rf "$d" && echo "== removed $d (idle; disk below 60 GB free)"
  done
}
