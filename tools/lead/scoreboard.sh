#!/bin/bash
# Is the way of working delivering? Per UTC day: commits landed on main per agent (subject prefix), reverts, watcher
# green/red runs and time to green, releases. Usage: scoreboard.sh [days=3]
D=${1:-3}; R=/root/projects/noshape; L=/tmp/lead/watch.log
git -C $R fetch -q origin --tags 2>/dev/null
for i in $(seq $((D-1)) -1 0); do
  day=$(date -u -d "-$i day" +%F); next=$(date -u -d "$day +1 day" +%F)
  echo "== $day"
  echo -n "  landed: "; git -C $R log origin/main --since="${day}T00:00:00Z" --until="${next}T00:00:00Z" --no-merges --format=%s \
    | grep -v "^lead: regenerate" | sed -E 's/^([a-z0-9]+): .*/\1/; t; s/.*/other/' | sort | uniq -c | sort -rn | awk '{printf "%s %s, ", $2, $1}'; echo
  echo "  reverts: $(git -C $R log origin/main --since="${day}T00:00:00Z" --until="${next}T00:00:00Z" --format=%s | grep -ci 'revert')"
  echo "  watcher: $(grep "^$day" $L 2>/dev/null | grep -c " GREEN ") green / $(grep "^$day" $L 2>/dev/null | grep -c " RED ") red, $(grep "^$day" $L 2>/dev/null | grep -c " REVERTED ") auto-reverts"
  echo "  releases: $(git -C $R tag -l 'v*' --format='%(refname:short) %(creatordate:short)' | awk -v d=$day '$2==d {printf "%s ", $1}')"
done
echo "== now: $(cat /tmp/lead/main-status 2>/dev/null | head -1); last-green $(git -C $R log -1 --format='%h %cr' last-green 2>/dev/null)"
echo "   T2: $(head -1 /tmp/lead/t2-status 2>/dev/null) $(grep -c FAILED /tmp/lead/t2-status 2>/dev/null) failed"
# Parity: rows of docs/parity/*.md marked as matching live vs with a known gap; and rows qa re-verified (dated).
git -C $R show origin/main:docs/parity >/dev/null 2>&1 && (cd $R && git archive origin/main docs/parity | tar -x -O 2>/dev/null) | python3 -c '
import sys,re
rows=[l for l in sys.stdin.read().splitlines() if l.startswith("|") and not re.match(r"^\|[ :|-]+\|?$",l)]
ok=sum(1 for l in rows if "✅" in l or "✓" in l); gap=sum(1 for l in rows if re.search(r"❌|✗|⚠|🔧|differs|missing|not yet",l))
ver=sum(1 for l in rows if re.search(r"verified|v0\.\d+\.\d+ / [0-9a-f]{7}",l))
print(f"   parity: {ok} of {len(rows)} checklist rows match live ({100*ok//max(len(rows),1)}%), {gap} with a known gap; {ver} rows re-verified by qa")'
