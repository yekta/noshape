#!/bin/bash
# land-relay.sh: the land daemon's hands on this machine (vibe4 has no GitHub credentials; tools/tiers/land-daemon.sh).
# Every 2 s: does the daemon's requests in vibe4:~/<dir>/outbox ("push <branch> <ref there> <sha>": fetches the
# checked stack from vibe4 and pushes it, answering "ok" or "rejected …"; "delete <branch>…": deletes queue branches),
# and every 10 s copies the results back: vibe4:~/land/results/<worktree>.log → /tmp/lead/land-results/ (land.sh
# --status, the lead's monitor). land.sh starts it when it isn't running.
# Usage: setsid -f bash /tmp/lead/land-relay.sh [<dir> (default land)] >> /tmp/lead/land-relay.log 2>&1
set -uo pipefail
dir=${1:-land}; host=${NOSHAPE_LAND_QUEUE_HOST:-vibe4}; repo=/root/projects/noshape
out=/tmp/lead/land-results; [ "$dir" = land ] || out=/tmp/lead/$dir-results
mkdir -p "$out" /tmp/lead
# Pushes to the URL: no remote-tracking ref to update in the agents' shared repo (a lock taken by an agent's fetch
# failed the push here though GitHub had taken it).
url=$(git -C "$repo" remote get-url origin)
exec 9>"/tmp/lead/land-relay-$dir.lock"; flock -n 9 || { echo "land-relay: already running for $dir"; exit 0; }
ctl="/tmp/lead/.land-relay-$dir.ssh"
S() { ssh -n -o BatchMode=yes -o ConnectTimeout=10 -o ControlMaster=auto -o ControlPath="$ctl" -o ControlPersist=120 "$host" "$@"; }
log() { echo "$(date -u '+%F %T') $*"; }
log "land relay $$ for $host:~/$dir"
i=0
while true; do
  # Requests not yet answered, oldest first; a push older than 240 s is stale (the daemon gave up at 300 s).
  reqs=$(S "cd ~/$dir/outbox 2>/dev/null && now=\$(date +%s) && for f in *.req; do [ -e \"\$f\" ] || continue; id=\${f%.req}; [ -e \$id.res ] && continue; echo \"\$(( now - \$(stat -c %Y \"\$f\") )) \$id \$(cat \"\$f\")\"; done" 2>/dev/null)
  while read -r age id op rest; do
    [ -n "${id:-}" ] || continue
    case $op in
      push)
        read -r branch ref sha <<< "$rest"
        if [ "$age" -gt 240 ]; then S "rm -f ~/$dir/outbox/$id.req"; log "$id: stale push of ${sha:0:8} dropped"; continue; fi
        if git -C "$repo" fetch -q "$host:projects/noshape" "+$ref:refs/land-relay/$id" 2>/tmp/lead/land-relay-fetch.err; then
          if msg=$(git -C "$repo" push -q "$url" "$sha:refs/heads/$branch" 2>&1); then res=ok; else res="rejected: $(echo "$msg" | grep -m1 -E 'rejected|error|fatal' | cut -c1-200)"; fi
        else res="rejected: fetch from $host failed: $(head -1 /tmp/lead/land-relay-fetch.err)"; fi
        git -C "$repo" update-ref -d "refs/land-relay/$id" 2>/dev/null
        S "echo '${res//\'/}' > ~/$dir/outbox/$id.res.tmp && mv ~/$dir/outbox/$id.res.tmp ~/$dir/outbox/$id.res"
        log "$id: push ${sha:0:8} to $branch: $res";;
      delete)
        git -C "$repo" push -q "$url" --delete $rest 2>/dev/null
        S "rm -f ~/$dir/outbox/$id.req";;
      *) S "rm -f ~/$dir/outbox/$id.req"; log "$id: unknown request '$op $rest' dropped";;
    esac
  done <<< "$reqs"
  if [ $((i % 5)) = 0 ]; then
    rsync -q -t -e "ssh -o BatchMode=yes -o ConnectTimeout=10 -o ControlPath=$ctl" "$host:$dir/results/*.log" "$out/" 2>/dev/null
  fi
  i=$((i + 1)); sleep 2
done
