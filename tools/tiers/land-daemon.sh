#!/bin/bash
# land-daemon.sh: the land queue on the land server (vibe4; docs/TESTS.md "Landing"). land.sh pushes a change to a
# queue/ branch, writes its job file here and returns at once (11-01: agents spent ~25 h a day waiting on their own
# lands). Whenever a slot frees, this daemon takes every queued change in arrival order, up to LAND_BATCH (4), stacks
# them on main (one that doesn't apply goes back as a rebase conflict), runs land-checks.sh once on the stack and
# pushes it if green. A red batch is split in halves that go first in the queue (tested in parallel slots, down to
# the change at fault, which is reported; the others land). Batches form only from what waits: none waits to fill one.
# Pushes go through tools/tiers/land-relay.sh on the agents' machine (vibe4 has no GitHub credentials), which also
# brings the results back to /tmp/lead/land-results/<worktree>.log.
# State is on disk (LAND_DIR, default ~/land): jobs/, queue/ (arrival order), splits/ (halves of red batches and
# re-tests, first), run/<batch>/ (claimed: jobs, pid, state, check.log), results/<worktree>.log and results.tsv,
# history.tsv (timings of each check). Restart it any time; checks in flight carry on. When idle it follows
# origin/main's version of itself. Stop: touch $LAND_DIR/stop.
# Start: setsid -f bash ~/land/bin/land-daemon.sh >> ~/land/daemon.log 2>&1 (land.sh starts it when it isn't running)
# LAND_TARGET (default main) and LAND_DIR let a second instance land on a test branch; LAND_CHECKS replaces
# tools/tiers/land-checks.sh (a stand-in, to try the daemon).
set -uo pipefail
LD=${LAND_DIR:-$HOME/land}; TARGET=${LAND_TARGET:-main}; MAXB=${LAND_BATCH:-4}
repo=$HOME/projects/noshape; me=$(readlink -f "$0"); bin=$(dirname "$me")
inst=$(basename "$LD")   # refs/landq/<inst>/… in the shared repo
. "$HOME/noshape-env.sh"
. "$bin/slots.sh"
export LAND_SLOTS=$(( $(nproc) / 5 > 2 ? $(nproc) / 5 : 2 ))
export CARGO_BUILD_JOBS=$(( $(nproc) / LAND_SLOTS + 1 ))
export GIT_COMMITTER_NAME=yekta GIT_COMMITTER_EMAIL=ngyekta@gmail.com
mkdir -p "$LD"/{jobs,queue,splits,run,results,outbox,done,bin}
log() { echo "$(date -u '+%F %T') $*"; }
alive() { [ -n "$1" ] && kill -0 "$1" 2>/dev/null; }
field() { local f="$LD/jobs/$2.job"; [ -f "$f" ] || f="$LD/done/$2.job"; sed -n "s/^$1=//p" "$f" 2>/dev/null | head -1; }
# A front batch (a red batch's half, a re-test, a batch whose worker died): named by its first job, so halves keep
# arrival order; claimed before any queued change, never merged with new ones.
front() { local kind=$1; shift; printf 'kind=%s\n' "$kind" > "$LD/splits/.$1.tmp"; printf '%s\n' "$@" >> "$LD/splits/.$1.tmp"; mv "$LD/splits/.$1.tmp" "$LD/splits/$1.batch"; }

# The relay (land-relay.sh on the agents' machine) does what needs GitHub credentials: "push <branch> <ref here> <sha>"
# answers "ok" or "rejected …" in outbox/<id>.res (waited for); "delete <branch>…" is fire and forget.
relay() {
  local id; id="$(date +%s%N)-$BASHPID"
  echo "$*" > "$LD/outbox/.$id"; mv "$LD/outbox/.$id" "$LD/outbox/$id.req"
  [ "$1" = push ] || return 0
  for _ in $(seq 150); do
    [ -s "$LD/outbox/$id.res" ] && { cat "$LD/outbox/$id.res"; rm -f "$LD/outbox/$id".*; return 0; }
    sleep 2
  done
  rm -f "$LD/outbox/$id.req"; echo "relay down (no answer in 300 s)"
}

# One result per change: results.tsv, the worktree's log (synced back for land.sh --status and the lead's monitor).
result() {   # result <key> LANDED <main sha> | FAILED <reason>  (detail lines in $d/detail)
  local key=$1 st=$2 what=$3 name sha subj queued now total line
  name=$(field name "$key"); sha=$(field sha "$key"); subj=$(field subject "$key"); queued=$(field queued "$key")
  now=$(date +%s); total=$(( now - queued ))
  printf '%s\t%s\t%s\t%s %s\t%s\t%s\t%s\t%s\t%s\n' "$(date -u '+%F %T')" "${sha:0:8}" "$name" "$st" "$what" \
    "$(( t_claim - queued ))" "$total" "$n" "$bid" "$subj" >> "$LD/results.tsv"
  if [ "$st" = LANDED ]; then
    line="LANDED ${sha:0:8} as ${what:0:8} on $TARGET ($(( total / 60 )) min after queuing, batch of $n): $subj"
  else
    line="FAILED ${sha:0:8} $what ($(( total / 60 )) min after queuing, batch of $n): $subj"
  fi
  { echo "$(date -u '+%F %T') $line"; [ "$st" = FAILED ] && [ -s "$d/detail" ] && sed 's/^/    | /' "$d/detail"; } >> "$LD/results/$name.log"
  [ "$st" = FAILED ] && [ -s "$d/detail" ] && cp "$d/detail" "$LD/results/$key.out"
  log "$bid: $name $line"
  relay delete "$(field ref "$key")"; git -C "$repo" update-ref -d "refs/landq/$inst/job/$key" 2>/dev/null
  mv "$LD/jobs/$key.job" "$LD/done/" 2>/dev/null
}

# Which of the given files are in crates the batch edits ("manifest" for Cargo.toml / Cargo.lock).
overlap() {   # overlap <moved files> <batch's changed files>
  python3 - "$1" "$2" <<'P'
import json, subprocess, sys, os
moved, changed = sys.argv[1].split('\n'), sys.argv[2].split('\n')
meta = json.loads(subprocess.run(['cargo', 'metadata', '--format-version', '1', '--no-deps'], capture_output=True, text=True).stdout)
root = meta['workspace_root']
dirs = [os.path.dirname(os.path.relpath(p['manifest_path'], root)) for p in meta['packages']]
crate = lambda f: 'manifest' if f in ('Cargo.toml', 'Cargo.lock') else next((d for d in dirs if f.startswith(d + '/')), None)
ours = {crate(c) for c in changed} - {None}
print(' '.join(sorted({crate(c) for c in moved if crate(c) in ours})))
P
}

# A batch, on a slot: stack, check, push or split.
run_batch() {
  local slot=$1 wt key keys=() applied=() conflict files other before commits c tip i k rc reason res moved hit t0 s
  d="$LD/run/$bid"; mapfile -t keys < <(grep -v '^kind=' "$d/jobs"); kind=$(sed -n 's/^kind=//p' "$d/jobs"); kind=${kind:-batch}
  n=${#keys[@]}; t0=$(date +%s)
  wt="$HOME/land/wt-slot$slot"; export CARGO_TARGET_DIR="$HOME/target-land-slot$slot"
  echo stacking > "$d/state"
  local refs=("+refs/heads/$TARGET:refs/landq/$inst/target")
  for key in "${keys[@]}"; do refs+=("+refs/heads/$(field ref "$key"):refs/landq/$inst/job/$key"); done
  git -C "$repo" fetch -q origin "${refs[@]}" 2>"$d/fetch.err" || {
    # One gone ref fails the whole fetch: fetch each, and report the ones that are gone.
    git -C "$repo" fetch -q origin "${refs[0]}" || { log "$bid: fetch of $TARGET failed"; sleep 30; front resume "${keys[@]}"; return; }
    for key in "${keys[@]}"; do git -C "$repo" fetch -q origin "+refs/heads/$(field ref "$key"):refs/landq/$inst/job/$key" 2>/dev/null \
      || { : > "$d/detail"; result "$key" FAILED "queue branch $(field ref "$key") is gone: re-run land.sh"; }; done
  }
  [ -d "$wt" ] || git -C "$repo" worktree add -q --detach "$wt" "refs/landq/$inst/target"
  # (A slot's checkout may be left mid cherry-pick or rebase by a killed batch.)
  cd "$wt" || { log "$bid: no checkout $wt"; sleep 30; front resume "${keys[@]}"; return; }
  { git cherry-pick --quit; git rebase --quit; } 2>/dev/null
  git checkout -q -f --detach "refs/landq/$inst/target" && git clean -fdq || { log "$bid: checkout failed"; sleep 30; front resume "${keys[@]}"; return; }
  base=$(git rev-parse HEAD); : > "$d/changed"; : > "$d/counts"; local pkgs=""
  for key in "${keys[@]}"; do
    [ -f "$LD/jobs/$key.job" ] || continue
    local jref="refs/landq/$inst/job/$key"
    git rev-parse -q --verify "$jref" > /dev/null || continue
    # Its commits not yet on the stack (by content: a change already landed by --wait or with an earlier job drops out).
    commits=$(git rev-list --reverse --cherry-pick --right-only HEAD..."$jref")
    # None: on main already (landed), or all in an earlier change of this batch (it lands or fails with them).
    if [ -z "$commits" ] && [ "$(git rev-parse HEAD)" = "$base" ]; then : > "$d/detail"; result "$key" LANDED "$base"; continue; fi
    before=$(git rev-parse HEAD)
    if [ -z "$commits" ]; then
      :
    elif git merge-base --is-ancestor HEAD "$jref" && [ "$(git rev-list --count HEAD.."$jref")" = "$(echo "$commits" | wc -l)" ]; then
      git merge -q --ff-only "$jref"   # (its own shas, as when it was rebased on this main)
    elif ! git cherry-pick $commits > /dev/null 2>&1; then
      files=$(git diff --name-only --diff-filter=U); git cherry-pick --abort 2>/dev/null; git reset -q --hard "$before"
      other=""
      for k in "${applied[@]}"; do
        echo "$files" | grep -qxF -f <(sed -n '/^--$/,$p' "$LD/jobs/$k.job" | tail -n +2) && other="$other $(field name "$k") ${k##*-}"
      done
      echo "$files" > "$d/detail"
      if [ -n "$other" ]; then reason="rebase conflict with$other, queued before it (in $(echo $files)): rebase on origin/$TARGET once that lands, then re-run land.sh"
      else reason="rebase conflict with $TARGET (in $(echo $files)): rebase on origin/$TARGET by hand, then re-run land.sh"; fi
      result "$key" FAILED "$reason"; continue
    fi
    applied+=("$key"); echo "$key $(git rev-list --count "$before"..HEAD)" >> "$d/counts"
    sed -n '/^--$/,$p' "$LD/jobs/$key.job" | tail -n +2 >> "$d/changed"; pkgs="$pkgs $(field pkgs "$key")"
  done
  n=${#applied[@]}; local t_stack=$(( $(date +%s) - t0 ))
  [ $n -gt 0 ] || return
  pkgs=$(echo $pkgs | tr ' ' '\n' | sort -u | tr '\n' ' '); sort -u -o "$d/changed" "$d/changed"
  echo "checking" > "$d/state"; log "$bid: $kind of $n on ${base:0:8} in slot $slot: $(for k in "${applied[@]}"; do printf '%s ' "$(field name "$k"):${k##*-}"; done)"
  prune_land_target "$CARGO_TARGET_DIR" > /dev/null
  : > "$d/times"; s=$(date +%s)
  LAND_TIMES="$d/times" timeout 90m ${LAND_CHECKS:-tools/tiers/land-checks.sh} "$pkgs" "$d/changed" > "$d/check.log" 2>&1; rc=$?
  local t_check=$(( $(date +%s) - s )) t_push=0 res_word
  if [ $rc -eq 0 ]; then
    echo "pushing" > "$d/state"; s=$(date +%s)
    while true; do
      git update-ref "refs/landq/$inst/push/$bid" HEAD
      res=$(relay push "$TARGET" "refs/landq/$inst/push/$bid" "$(git rev-parse HEAD)")
      git update-ref -d "refs/landq/$inst/push/$bid"
      [ "$res" = ok ] && break
      log "$bid: push: $res"
      if [ "$res" != "${res#relay down}" ]; then echo "pushing (waiting for the relay)" > "$d/state"; sleep 30; fi
      # Main moved while we checked (a docs-only push, the lead): rebase; check again only if the new commits touch
      # a crate this batch edits (10-27: a `far` landed in kernel beside the kernel change that made it illegal).
      git -C "$repo" fetch -q origin "+refs/heads/$TARGET:refs/landq/$inst/target"
      git merge-base --is-ancestor HEAD "refs/landq/$inst/target" && { res=ok; break; }   # (it got there after all)
      moved=$(git diff --name-only "$base" "refs/landq/$inst/target")
      [ -n "$moved" ] || sleep 10   # (rejected though main didn't move: GitHub or the relay; try again shortly)
      hit=$(overlap "$moved" "$(cat "$d/changed")")
      if [ -n "$hit" ] || ! git rebase -q "refs/landq/$inst/target" > /dev/null 2>&1; then
        git rebase --abort 2>/dev/null
        if [ -n "$hit" ]; then log "$bid: again from the front ($TARGET changed $hit while checking)"; else log "$bid: again from the front (rebase conflict after checking)"; fi
        front retest "${applied[@]}"; res_word=retest; break
      fi
      base=$(git rev-parse "refs/landq/$inst/target")
    done
    t_push=$(( $(date +%s) - s ))
    if [ "$res" = ok ]; then
      res_word=landed
      # Each change's own tip on main (a rebase that dropped a commit leaves HEAD for all).
      mapfile -t c < <(git rev-list --reverse "$base"..HEAD); i=0
      while read -r k cnt; do
        i=$(( i + cnt )); tip=${c[$((i - 1))]:-}; [ ${#c[@]} -eq "$(awk '{s+=$2} END {print s}' "$d/counts")" ] || tip=$(git rev-parse HEAD)
        : > "$d/detail"; result "$k" LANDED "$tip"
      done < "$d/counts"
    fi
  else
    reason=$(sed -n 's/^LAND FAILED: //p' "$d/check.log" | tail -1); [ $rc -eq 124 ] && reason="timeout (checks over 90 min)"
    reason=${reason:-"checks exited $rc"}
    if [ $n -eq 1 ]; then
      res_word=failed
      grep -vE '^(== |CHECKS PASSED)' "$d/check.log" | grep -v '^LAND FAILED' | tail -60 > "$d/detail"
      result "${applied[0]}" FAILED "$reason"
    else
      # Red: halves go first, each on main as it is then; the one at fault is the half that stays red, down to one.
      res_word=split; local h=$(( (n + 1) / 2 ))
      log "$bid: red ($reason): splitting $n into $h + $(( n - h ))"
      front split "${applied[@]:0:$h}"; front split "${applied[@]:$h}"
    fi
  fi
  # Timings of this check (docs/TESTS.md "Landing"): queue wait = queued → claimed, the longest in the batch.
  local qw=0 q; for k in "${applied[@]}"; do q=$(( t_claim - $(field queued "$k") )); [ $q -gt $qw ] && qw=$q; done
  [ -s "$LD/history.tsv" ] || printf 'end\thost\tbatch\tkind\tsize\tresult\tqueue_wait\tslot_wait\tstack\tcheck_total\tcheck\tclippy\ttests_build\ttests_run\tt0\tmenu\tpush\treason\tcrates\n' > "$LD/history.tsv"
  tm() { awk -v p="$1" '$1 == p {print $2}' "$d/times"; }
  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$(date -u '+%F %T')" "$(hostname -s)" "$bid" "$kind" "$n" \
    "$res_word" "$qw" "$SLOT_WAITED" "$t_stack" "$t_check" "$(tm check)" "$(tm clippy)" "$(tm tests_build)" "$(tm tests_run)" "$(tm t0)" "$(tm menu)" \
    "$t_push" "$( [ $rc -eq 0 ] || echo "$reason")" "$(echo $pkgs | wc -w)" >> "$LD/history.tsv"
}

# A worker: waits for a slot in arrival order with perf runs and --wait lands (slots.sh), then claims what is queued.
worker() {
  local tok=$1 slot
  take_slots 1; slot=${SLOT_IDS[0]}
  trap 'rm -f "$tok"; [ -z "$bid" ] || rm -rf "${LD:?}/run/$bid"' EXIT
  bid=""
  {
    flock 5
    local b; b=$(ls "$LD"/splits/*.batch 2>/dev/null | head -1)
    if [ -n "$b" ]; then
      bid="$(date -u +%m%d-%H%M%S)-$BASHPID"; mkdir -p "$LD/run/$bid"; echo "$BASHPID" > "$LD/run/$bid/pid"; mv "$b" "$LD/run/$bid/jobs"
    else
      mapfile -t ks < <(ls "$LD/queue" | head -"$MAXB")
      if [ ${#ks[@]} -gt 0 ]; then
        bid="$(date -u +%m%d-%H%M%S)-$BASHPID"; mkdir -p "$LD/run/$bid"; echo "$BASHPID" > "$LD/run/$bid/pid"
        { echo kind=batch; printf '%s\n' "${ks[@]}"; } > "$LD/run/$bid/jobs"
        for k in "${ks[@]}"; do rm -f "$LD/queue/$k"; done
      fi
    fi
  } 5>"$LD/claim.lock"
  rm -f "$tok"
  [ -n "$bid" ] || return 0
  t_claim=$(date +%s)
  run_batch "$slot"
}

# Batches whose worker died (a reboot, a kill -9) go first again; busy = the batches being checked.
recover() {
  local r p ks; busy=0
  for r in "$LD"/run/*/; do
    [ -d "$r" ] || continue
    p=$(cat "$r/pid" 2>/dev/null); { [ -z "$p" ] || alive "$p"; } && { busy=$((busy + 1)); continue; }
    mapfile -t ks < <(grep -v '^kind=' "$r/jobs" 2>/dev/null | while read -r k; do [ -f "$LD/jobs/$k.job" ] && echo "$k"; done)
    [ ${#ks[@]} -gt 0 ] && { front resume "${ks[@]}"; log "resumed $(basename "$r") (${#ks[@]} changes; its worker is gone)"; }
    rm -rf "$r"
  done
}

# The daemon: one per LAND_DIR.
[ "${1:-}" = "" ] || { echo "usage: land-daemon.sh  (env LAND_DIR, LAND_TARGET, LAND_BATCH, LAND_SELF_UPDATE=0)"; exit 2; }
exec 9>"$LD/daemon.lock"; flock -n 9 || { echo "land-daemon: already running for $LD"; exit 0; }
echo $$ > "$LD/daemon.pid"; rm -f "$LD/stop"
log "land daemon $$ for $LD → $TARGET, batches up to $MAXB, $LAND_SLOTS slots ($(sha1sum < "$me" | cut -c1-8))"
rm -f "$LD"/waiting.*
last_update=0
while [ ! -e "$LD/stop" ]; do
  recover; waiting=0
  for w in "$LD"/waiting.*; do
    [ -e "$w" ] || continue
    p=$(cat "$w" 2>/dev/null); if [ -z "$p" ] || alive "$p"; then waiting=$((waiting + 1)); else rm -f "$w"; fi
  done
  # Idle: follow origin's version of this daemon (checked each minute).
  if [ $busy = 0 ] && [ $waiting = 0 ] && [ "${LAND_SELF_UPDATE:-1}" = 1 ] && [ $(( $(date +%s) - last_update )) -ge 60 ]; then
    last_update=$(date +%s)
    if git -C "$repo" fetch -q origin "+refs/heads/$TARGET:refs/landq/$inst/self" 2>/dev/null; then
      new=$(git -C "$repo" show "refs/landq/$inst/self:tools/tiers/land-daemon.sh" 2>/dev/null)
      if [ -n "$new" ] && [ "$new" != "$(cat "$me")" ] && bash -n <(echo "$new"); then
        git -C "$repo" show "refs/landq/$inst/self:tools/tiers/slots.sh" > "$bin/slots.sh.new" && mv "$bin/slots.sh.new" "$bin/slots.sh"
        echo "$new" > "$me.new" && mv "$me.new" "$me"
        log "updated from origin/$TARGET: restarting"; exec 9>&-; exec bash "$me"
      fi
    fi
  fi
  if [ $waiting = 0 ] && [ $busy -lt "$LAND_SLOTS" ] && { ls "$LD"/splits/*.batch > /dev/null 2>&1 || [ -n "$(ls "$LD/queue")" ]; }; then
    tok="$LD/waiting.$(date +%s%N)"; : > "$tok"
    ( exec 9>&-; worker "$tok" ) >> "$LD/daemon.log" 2>&1 &
    echo $! > "$tok"
  fi
  sleep 2
done
log "stopped ($LD/stop)"; rm -f "$LD/stop"
