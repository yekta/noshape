#!/bin/bash
# Ensure the main watcher and disk loop are running (exact match, so this script's own command line can't fool it).
for s in disk-loop watch-main; do
  if pgrep -fx "/bin/bash /tmp/lead/$s.sh" >/dev/null; then echo "$s: running"
  else cd /tmp && setsid nohup /bin/bash /tmp/lead/$s.sh >/dev/null 2>&1 </dev/null & echo "$s: RESTARTED"; fi
done
# A queue killed mid-publish leaves local main green but unpushed.
cd /root/noshape && git fetch -q origin 2>/dev/null
if ! git merge-base --is-ancestor refs/heads/main refs/remotes/origin/main; then
  git merge-base --is-ancestor refs/remotes/origin/main refs/heads/main && git push -q origin main && echo "main: PUSHED unpushed $(git rev-parse --short main)"
fi
