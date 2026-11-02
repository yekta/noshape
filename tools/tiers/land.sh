#!/bin/bash
# land.sh: the only way to put work on main (docs/TESTS.md "Landing"). Rebases on origin/main, queues HEAD on the land
# server (vibe4) and returns at once ("QUEUED <sha>"): keep working. The land daemon there (tools/tiers/land-daemon.sh)
# checks queued changes in batches (cargo check over the workspace, clippy on the crates you edit, the full tests of
# those crates and of the crates that use them directly, T0 when the app or the agent API is affected), pushes the
# green ones to main and finds the change at fault in a red batch (11-01: agents spent ~25 h a day waiting on their
# own lands). Your results: `land.sh --status`, or the lead's message. A failed land is your next task: fix forward
# (work you stacked on it stays in your worktree; the next land.sh queues all of it). Docs-only changes push at once.
#   tools/tiers/land.sh            queue HEAD, once clippy is clean on the crates it edits (run through rb.sh in your
#                                  warm vibe4 dir, ~1 min; a clippy error refuses the queue, with the error)
#   tools/tiers/land.sh --no-precheck   queue without that clippy run (emergencies)
#   tools/tiers/land.sh --status   this worktree's queued, landed and failed changes
#   tools/tiers/land.sh --wait     land now and wait (the lead, scripts; also when vibe4 is unreachable): this change's
#                                  checks alone on vibe4, else vibe3, else here (env: OCCT_ROOT, CARGO_TARGET_DIR,
#                                  CC/CXX), then the push. Re-run after a rejected push (main moved).
# NOSHAPE_LAND_TARGET (default main) and NOSHAPE_LAND_DIR (the daemon's dir on vibe4, default land) try a test branch.
set -uo pipefail
cd "$(git rev-parse --show-toplevel)"
fail() { echo "LAND FAILED: $*"; exit 1; }
mode=queue; precheck=1
for a in "$@"; do
  case "$a" in --wait) mode=wait;; --status) mode=status;; --no-precheck) precheck=0;; *) sed -n '2,/^set /p' "$0" | grep '^#'; exit 2;; esac
done
name=$(basename "$PWD"); target=${NOSHAPE_LAND_TARGET:-main}
qhost=${NOSHAPE_LAND_QUEUE_HOST:-vibe4}; qdir=${NOSHAPE_LAND_DIR:-land}
results=/tmp/lead/land-results/$name.log; [ "$qdir" = land ] || results=/tmp/lead/$qdir-results/$name.log
SSH=(ssh -o BatchMode=yes -o ConnectTimeout=5)

# On the land server: this worktree's changes being checked or queued, then its latest results.
read -r -d '' STATUS <<'R'
LD=~/$1; me=$2; cd "$LD" 2>/dev/null || exit 0
f() { sed -n "s/^$1=//p" "jobs/$2.job" 2>/dev/null | head -1; }
line() { echo "$1 $(f sha "$2" | cut -c1-8) (queued $(date -u -d @"$(f queued "$2")" +%H:%M) UTC): $(f subject "$2")"; }
for r in run/*/; do
  [ -f "$r/jobs" ] || continue; n=$(grep -vc '^kind=' "$r/jobs")
  for k in $(grep -v '^kind=' "$r/jobs"); do [ "$(f name "$k")" = "$me" ] && line "CHECKING ($(cat "$r/state" 2>/dev/null), in a batch of $n since $(date -u -r "$r/jobs" +%H:%M))" "$k"; done
done
for b in splits/*.batch; do [ -f "$b" ] || continue; for k in $(grep -v '^kind=' "$b"); do [ "$(f name "$k")" = "$me" ] && line "QUEUED (next: re-checked apart from a red batch)" "$k"; done; done
for c in jobs/*.carry; do [ -f "$c" ] || continue; for k in $(cat "$c"); do [ "$(f name "$k")" = "$me" ] && line "QUEUED (carried by your later $(basename "$c" .carry | sed 's/.*-//'))" "$k"; done; done
pos=0; for k in $(ls pending); do pos=$((pos + 1)); [ "$(f name "$k")" = "$me" ] && line "QUEUED (#$pos)" "$k"; done
echo "==results=="; tail -n 25 "results/$me.log" 2>/dev/null; exit 0
R
if [ $mode = status ]; then
  if out=$("${SSH[@]}" "$qhost" "bash -c $(printf %q "$STATUS") _ $qdir $name" 2>/dev/null); then
    live=$(echo "$out" | sed '/^==results==$/,$d'); echo "${live:-nothing queued or being checked}"
    echo "$out" | sed '0,/^==results==$/d'
  else
    echo "($qhost unreachable: the latest results synced here)"; tail -n 25 "$results" 2>/dev/null
  fi
  exit 0
fi

# Results that came in since this worktree last ran land.sh: failures first.
seen="$(git rev-parse --git-dir)/land-seen"
if [ -f "$results" ]; then
  new=$(tail -n +"$(( $(cat "$seen" 2>/dev/null || echo 0) + 1 ))" "$results" | grep -E '^[0-9-]+ [0-9:]+ (LANDED|FAILED)' )
  [ -n "$new" ] && { echo "== since your last land:"; echo "$new" | sed 's/^/   /'; }
  wc -l < "$results" > "$seen"
fi

[ -z "$(git status --porcelain --untracked-files=no)" ] || fail "uncommitted changes: commit them first"
git fetch -q origin || fail "fetch"
git rebase -q "origin/$target" || { git rebase --abort; fail "rebase conflict: rebase by hand, then re-run"; }
changed=$(git diff --name-only "origin/$target...HEAD")
[ -n "$changed" ] || { echo "nothing to land"; exit 0; }
# A squash onto a stale base reverts other agents' work (bool 11-02: 54,689 lines): refuse big deletions.
deleted=$(git diff --numstat "origin/$target...HEAD" | awk '$2 != "-" {d += $2} END {print d + 0}')
[ "$deleted" -le 3000 ] || [ "${LAND_BIG_DELETE:-0}" = 1 ] ||
  fail "deletes $deleted lines (git diff --stat origin/$target...HEAD): a stale squash? LAND_BIG_DELETE=1 if meant"
# Docs-only changes: push without building.
if ! echo "$changed" | grep -qvE '^(docs/|.*\.md$)'; then
  git push -q origin "HEAD:$target" && { echo "LANDED (docs only) $(git rev-parse --short HEAD)"; exit 0; } || fail "push rejected: re-run"
fi
# Crates touched, then the workspace crates that depend on them directly.
pkgs=$(python3 - "$changed" <<'P'
import json, subprocess, sys, os
changed = sys.argv[1].split('\n')
meta = json.loads(subprocess.run(['cargo', 'metadata', '--format-version', '1', '--no-deps'], capture_output=True, text=True).stdout)
root = meta['workspace_root']
pk = {p['name']: os.path.dirname(os.path.relpath(p['manifest_path'], root)) for p in meta['packages']}
deps = {p['name']: {d['name'] for d in p['dependencies'] if d['name'] in pk} for p in meta['packages']}
touched = {n for n, d in pk.items() if any(c == d or c.startswith(d + '/') for c in changed)}
if any(c in ('Cargo.toml', 'Cargo.lock') or c.startswith('.cargo/') for c in changed): touched = set(pk)
out = set(touched) | {n for n, ds in deps.items() if ds & touched}
out.discard('noshape-workspace-hack')
print(' '.join(sorted(out)))
P
)
[ -n "$pkgs" ] || pkgs="noshape-ops"

if [ $mode = queue ] && ! "${SSH[@]}" "$qhost" true 2>/dev/null; then
  echo "== $qhost unreachable: landing now and waiting (--wait)"; mode=wait
fi
# Before queuing: clippy on the crates the change edits, in this worktree's warm dir on vibe4 (tools/tiers/rb.sh). Most
# failed queued changes on 11-01 (~60; dx 23 failed, 6 landed) were one clippy error or failing test carried through an
# agent's next commits; now a stack queues only once its newest commit is clippy-clean.
if [ $mode = queue ] && [ $precheck = 1 ]; then
  edited=$(python3 - "$changed" <<'P'
import json, subprocess, sys, os
changed = sys.argv[1].split('\n')
meta = json.loads(subprocess.run(['cargo', 'metadata', '--format-version', '1', '--no-deps'], capture_output=True, text=True).stdout)
root = meta['workspace_root']
pk = {p['name']: os.path.dirname(os.path.relpath(p['manifest_path'], root)) for p in meta['packages']}
if any(c in ('Cargo.toml', 'Cargo.lock') or c.startswith('.cargo/') for c in changed): print('all'); sys.exit()
print(' '.join(sorted(n for n, d in pk.items() if n != 'noshape-workspace-hack' and any(c.startswith(d + '/') for c in changed))))
P
)
  if [ -n "$edited" ]; then
    if [ "$edited" = all ]; then cl=(cargo clippy --workspace --all-targets -- -D warnings); else cl=(cargo clippy --no-deps $(for p in $edited; do printf -- '-p %s ' "$p"; done) --all-targets -- -D warnings); fi
    echo "== precheck: clippy on ${edited} (vibe4, your warm dir)"; s=$(date +%s)
    out=$(NOSHAPE_RB_HOST=$qhost tools/tiers/rb.sh "${cl[@]}" 2>&1); rc=$?
    if [ $rc -ne 0 ]; then
      echo "$out" | grep -vE '^warning: [a-z0-9_-]+@' | grep -E '^(error|warning)' -A6 | head -60
      fail "precheck: clippy (not queued; fix it and land again, or --no-precheck in an emergency)"
    fi
    echo "== precheck passed ($(( $(date +%s) - s )) s)"
  fi
fi
if [ $mode = queue ]; then
  sha=$(git rev-parse HEAD); short=${sha:0:8}; ref="queue/$name-$short"; key="$(date +%s%N)-$name-$short"
  # (To the URL: no remote-tracking ref to lock here, where all agents' worktrees share one repo; 11-01 parallel lands
  # failed on that lock.)
  url=$(git remote get-url origin); for i in 1 2 3; do git push -q -f "$url" "HEAD:refs/heads/$ref" 2>/dev/null && break; [ $i = 3 ] && fail "push to $ref"; sleep $i; done
  # The relay here does the daemon's pushes (vibe4 has no GitHub credentials) and syncs the results back.
  mkdir -p /tmp/lead
  if flock -n "/tmp/lead/land-relay-$qdir.lock" true 2>/dev/null; then
    # (In /root/lead-scratch, not /tmp/lead: it survives a reboot, and the lead's monitor restarts it from there.)
    rl=/root/lead-scratch/land-relay.sh; mkdir -p /root/lead-scratch
    git show "origin/$target:tools/tiers/land-relay.sh" > "$rl.new" 2>/dev/null && mv "$rl.new" "$rl"
    log=/root/lead-scratch/land-relay.log; [ "$qdir" = land ] || log=/root/lead-scratch/land-relay-$qdir.log
    [ -s "$rl" ] && setsid -f bash "$rl" "$qdir" >> "$log" 2>&1 < /dev/null
  fi
  # The job (arrival order = its name), then the daemon if it isn't running (origin's version).
  read -r -d '' REG <<'R'
LD=~/$1; key=$2; target=$3; name=${key#*-}; name=${name%-*}
ls "$LD/jobs/"*"-${key#*-}.job" > /dev/null 2>&1 && { cat > /dev/null; echo ALREADY; exit 0; }
mkdir -p "$LD/jobs" "$LD/pending" "$LD/bin" && cat > "$LD/jobs/.$key" && mv "$LD/jobs/.$key" "$LD/jobs/$key.job" || exit 1
# This worktree's changes still waiting to be claimed are in this one (it queues everything not on main): it carries
# them, so one agent's run of lands takes one place in a batch and one result (11-01: skasm's four took a batch and
# three rounds of bisection). The daemon checks they really are in it.
exec 5>"$LD/claim.lock"; flock 5; carried=0
for p in $(ls "$LD/pending"); do
  [ "$(sed -n 's/^name=//p' "$LD/jobs/$p.job" 2>/dev/null)" = "$name" ] && [ ! -e "$LD/jobs/$p.solo" ] || continue
  rm -f "$LD/pending/$p"; { echo "$p"; cat "$LD/jobs/$p.carry" 2>/dev/null; } >> "$LD/jobs/$key.carry"; rm -f "$LD/jobs/$p.carry"; carried=$((carried + 1))
done
: > "$LD/pending/$key"; exec 5>&-
[ $carried = 0 ] || echo "(it carries your $carried earlier queued change(s): one result for all)"
if ! { [ -f "$LD/daemon.pid" ] && kill -0 "$(cat "$LD/daemon.pid")" 2>/dev/null; }; then
  cd ~/projects/noshape && git fetch -q origin "$target" && git show "origin/$target:tools/tiers/land-daemon.sh" > "$LD/bin/land-daemon.sh.new" 2>/dev/null \
    && git show "origin/$target:tools/tiers/slots.sh" > "$LD/bin/slots.sh" && mv "$LD/bin/land-daemon.sh.new" "$LD/bin/land-daemon.sh" \
    && LAND_DIR=$LD LAND_TARGET=$target setsid -f bash "$LD/bin/land-daemon.sh" >> "$LD/daemon.log" 2>&1 < /dev/null && echo "(started the land daemon on $(hostname -s))"
fi
echo "$(( $(ls "$LD/pending" | wc -l) - 1 )) queued before it, $(ls -d "$LD"/run/*/ 2>/dev/null | wc -l) batch(es) being checked"
R
  info=$(printf 'name=%s\nsha=%s\nbase=%s\nref=%s\npkgs=%s\nqueued=%s\nsubject=%s\n--\n%s\n' "$name" "$sha" "$(git rev-parse "origin/$target")" \
    "$ref" "$pkgs" "$(date +%s)" "$(git log -1 --format=%s | tr -d '\t')" "$changed" \
    | "${SSH[@]}" "$qhost" "bash -c $(printf %q "$REG") _ $qdir $key $target") || fail "could not queue on $qhost: re-run, or land with --wait"
  [ "$info" = ALREADY ] && { echo "ALREADY QUEUED $short: nothing new since (tools/tiers/land.sh --status)"; exit 0; }
  echo "QUEUED $short ($(git rev-list --count "origin/$target..HEAD") commit(s); crates: $pkgs)"
  echo "$info" | sed 's/^/   /'
  echo "   Keep working. Results: tools/tiers/land.sh --status (or the lead tells you)."
  exit 0
fi

# --wait: this change's checks alone, then the push.
echo "== crates: $pkgs"
# The checks (tools/tiers/land-checks.sh) run on the land server (vibe4, else vibe3), so agents' full test runs don't
# load this machine (10-28): the commit goes to a temporary land/ branch, the server checks it in one of its slots.
# NOSHAPE_LAND_LOCAL=1, or no land server reachable: here.
cf=$(mktemp); echo "$changed" > "$cf"
host=""; [ "${NOSHAPE_LAND_LOCAL:-0}" = 1 ] || for h in ${NOSHAPE_LAND_HOSTS:-vibe4 vibe3}; do "${SSH[@]}" "$h" true 2>/dev/null && { host=$h; break; }; done
if [ -n "$host" ]; then
  branch="land/$name-$(git rev-parse --short HEAD)"
  git push -q -f origin "HEAD:refs/heads/$branch" 2>/dev/null || fail "push to $branch"
  ssh -o BatchMode=yes "$host" "mkdir -p ~/land && cd ~/projects/noshape && git fetch -q origin $branch && git show FETCH_HEAD:tools/tiers/land-remote.sh > ~/land/remote-$name.sh && bash ~/land/remote-$name.sh '$name' '$branch' '$pkgs'" < "$cf"
  rc=$?; git push -q origin --delete "$branch" 2>/dev/null
else
  echo "== here (NOSHAPE_LAND_LOCAL=1 or no land server reachable)"
  tools/tiers/land-checks.sh "$pkgs" "$cf"; rc=$?
fi
rm -f "$cf"
[ $rc -eq 0 ] || exit 1
# Main may have moved while we tested: rebase again; re-test only if the new commits touch the crates we edit.
tested=$(git rev-parse "origin/$target")
git fetch -q origin
if ! git merge-base --is-ancestor "origin/$target" HEAD; then
  moved=$(git diff --name-only "$tested" "origin/$target")
  git rebase -q "origin/$target" || { git rebase --abort; fail "rebase conflict after testing"; }
  # A commit in a crate this change edits wasn't tested with it (10-27: a `far` landed in kernel beside the kernel
  # change that made `far` illegal, and main went red): test again. Clashes across crates are rarer; the watcher catches them.
  hit=$(python3 - "$moved" "$changed" <<'P'
import json, subprocess, sys, os
moved, changed = sys.argv[1].split('\n'), sys.argv[2].split('\n')
meta = json.loads(subprocess.run(['cargo', 'metadata', '--format-version', '1', '--no-deps'], capture_output=True, text=True).stdout)
root = meta['workspace_root']
dirs = [os.path.dirname(os.path.relpath(p['manifest_path'], root)) for p in meta['packages']]
crate = lambda f: 'manifest' if f in ('Cargo.toml', 'Cargo.lock') else next((d for d in dirs if f.startswith(d + '/')), None)
ours = {crate(c) for c in changed} - {None}
print(' '.join(c for c in moved if crate(c) in ours))
P
)
  [ -z "$hit" ] || fail "main changed the crates you edit while testing ($hit): re-run land.sh --wait"
fi
git push -q origin "HEAD:$target" || fail "push rejected (main moved): re-run land.sh --wait"
echo "LANDED $(git rev-parse --short HEAD)"
