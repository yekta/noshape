# The land server's two slots (~/land/slot1, slot2), taken in arrival order. Sourced by land-remote.sh (one slot)
# and perf-remote.sh (both: the machine to itself). Before 10-31 lands polled the slots every 5 s and perf runs
# waited in flock, so whoever looked at the right moment went first: one land waited 70 min while later ones went.
# Now each waiter appends itself to ~/land/queue and only the first live entry may take slots; it leaves the queue
# once it holds them, so the next one takes the other slot.
# take_slots N: returns holding N slots (on fds 8 / 9; the caller closes both when done); SLOT_WAITED = seconds waited.
take_slots() {
  local n=$1 q="$HOME/land/queue" me first h1=0 h2=0
  me="$(date +%s%N) $$"; SLOT_WAITED=0
  mkdir -p "$HOME/land"; touch "$q"
  { flock 4; echo "$me" >> "$q"; } 4>"$q.lock"
  # Gone from the queue however this script ends (a killed waiter's entry is also dropped below: its pid is dead).
  trap '{ flock 4; grep -vxF "'"$me"'" "'"$q"'" > "'"$q"'.tmp"; mv "'"$q"'.tmp" "'"$q"'"; } 4>"'"$q"'.lock"' EXIT
  while true; do
    first=$({ flock 4; awk '{ if (system("kill -0 " $2 " 2>/dev/null") == 0) print }' "$q" > "$q.tmp"; mv "$q.tmp" "$q"; head -1 "$q"; } 4>"$q.lock")
    if [ "$first" = "$me" ]; then
      # (A slot taken in an earlier round is kept: perf waits holding one for the other, ahead of later arrivals.)
      if [ $h1 = 0 ]; then exec 8>"$HOME/land/slot1"; if flock -n 8; then h1=1; else exec 8>&-; fi; fi
      if [ $h2 = 0 ] && { [ "$n" = 2 ] || [ $h1 = 0 ]; }; then
        exec 9>"$HOME/land/slot2"; if flock -n 9; then h2=1; else exec 9>&-; fi
      fi
      if [ $((h1 + h2)) -ge "$n" ]; then
        { flock 4; grep -vxF "$me" "$q" > "$q.tmp"; mv "$q.tmp" "$q"; } 4>"$q.lock"
        trap - EXIT; return 0
      fi
    fi
    sleep 5; SLOT_WAITED=$((SLOT_WAITED + 5))
  done
}
