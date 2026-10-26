#!/bin/bash
# Ensure the main watcher and disk loop are running (exact match, so this script's own command line can't fool it).
for s in disk-loop watch-main; do
  if pgrep -fx "/bin/bash /tmp/lead/$s.sh" >/dev/null; then echo "$s: running"
  else cd /tmp && setsid nohup /bin/bash /tmp/lead/$s.sh >/dev/null 2>&1 </dev/null & echo "$s: RESTARTED"; fi
done
# Keep Motile's repo's local main equal to origin/main (its "Pull main" fails when local main has commits origin lacks).
# Fast-forward only; nobody commits on that local main.
cd /root/projects/noshape && git fetch -q origin 2>/dev/null
if [ "$(git branch --show-current)" = main ] && git merge-base --is-ancestor main origin/main; then git merge -q --ff-only origin/main 2>/dev/null
elif ! git merge-base --is-ancestor main origin/main; then echo "main: /root/projects/noshape main has commits origin/main lacks: check it"; fi
