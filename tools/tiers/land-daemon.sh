#!/bin/bash
# land-daemon.sh: the land queue on the land server (vibe4; docs/TESTS.md "Landing"). land.sh pushes a change to a
# queue/ branch, writes its job file here and returns at once (11-01: agents spent ~25 h a day waiting on their own
# lands). Whenever a slot frees, this daemon takes every queued change in arrival order, up to LAND_BATCH (4), stacks
# them on main (one that doesn't apply goes back as a rebase conflict), runs land-checks.sh once on the stack and
# pushes it if green. A red batch is split in halves that go first in the queue (tested in parallel slots, down to
# the change at fault, which is reported; the others land). Batches form only from what waits: none waits to fill one.
# Pushes go through tools/tiers/land-relay.sh on the agents' machine (vibe4 has no GitHub credentials), which also
# brings the results back to /tmp/lead/land-results/<worktree>.log.
# State is on disk (LAND_DIR, default ~/land): jobs/, pending/ (arrival order; ~/land/queue is slots.sh's file),
# splits/ (halves of red batches and re-tests, first), run/<batch>/ (claimed: jobs, pid, state, check.log), results/<worktree>.log and results.tsv,
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
# Slots: one per ~5 cores, at most one per ~12 GB of memory (vibe5, 16 cores and 29 GB, OOM-killed builds at 3).
cpu_slots=$(( $(nproc) / 5 > 2 ? $(nproc) / 5 : 2 ))
mem_slots=$(( $(awk '/MemTotal/ { print int($2 / 1048576) }' /proc/meminfo) / 12 ))
export LAND_SLOTS=${LAND_SLOTS:-$(( mem_slots < 1 ? 1 : (mem_slots < cpu_slots ? mem_slots : cpu_slots) ))}
export CARGO_BUILD_JOBS=$(( $(nproc) / LAND_SLOTS + 1 ))
export GIT_COMMITTER_NAME=yekta GIT_COMMITTER_EMAIL=ngyekta@gmail.com
mkdir -p "$LD"/{jobs,pending,splits,run,results,outbox,done,bin,outcome,failed} || { echo "land-daemon: can't make its dirs in $LD"; exit 1; }
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
  # The worktree's earlier queued changes this one carries (land.sh: queued again before they were claimed) get its result.
  local c; for c in $(cat "$LD/jobs/$key.carry" 2>/dev/null); do result "$c" "$st" "$what"; done; rm -f "$LD/jobs/$key.carry"
  if [ "$st" = FAILED ] && [ "${what#superseded}" = "$what" ]; then
    echo "$key $sha $now" >> "$LD/failed/$name"; supersede "$key"
  fi
  relay delete "$(field ref "$key")"; git -C "$repo" update-ref -d "refs/landq/$inst/job/$key" 2>/dev/null
  mv "$LD/jobs/$key.job" "$LD/done/" 2>/dev/null; rm -f "$LD/jobs/$key.solo"
}

# A failed change's worktree: its queued changes that contain it (queued before it failed, so not a fix on top) would
# fail the same way: they fail with it, unchecked (11-01: skasm queued 13 commits in a row; each one after a failure
# failed the same test again). Waiting ones and front batches' ones; not those being checked.
supersede() {   # supersede <failed key>
  local f=$1 name sha q b ok=() fref="refs/landq/$inst/job/$1"
  git -C "$repo" rev-parse -q --verify "$fref" > /dev/null || return 0
  name=$(field name "$f"); sha=$(field sha "$f")
  for q in $(ls "$LD/pending"; cat "$LD"/splits/*.batch 2>/dev/null | grep -v '^kind='); do
    [ "$q" != "$f" ] && [ "$(field name "$q")" = "$name" ] || continue
    git -C "$repo" fetch -q origin "+refs/heads/$(field ref "$q"):refs/landq/$inst/job/$q" 2>/dev/null || continue
    contains "refs/landq/$inst/job/$q" "$fref" && ok+=("$q")
  done
  [ ${#ok[@]} -gt 0 ] || return 0
  {
    flock 5
    for q in "${ok[@]}"; do
      rm -f "$LD/pending/$q"
      for b in "$LD"/splits/*.batch; do
        [ -f "$b" ] && grep -qx "$q" "$b" || continue
        grep -vx "$q" "$b" > "$b.tmp"; if grep -qv '^kind=' "$b.tmp"; then mv "$b.tmp" "$b"; else rm -f "$b" "$b.tmp"; fi
      done
    done
  } 5>"$LD/claim.lock"
  cp "$d/detail" "$d/detail.keep" 2>/dev/null; : > "$d/detail"
  for q in "${ok[@]}"; do result "$q" FAILED "superseded: contains failed ${sha:0:8} (fix it on top, then land.sh)"; done
  mv "$d/detail.keep" "$d/detail" 2>/dev/null
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

# The change at fault in a red batch, from the error (11-01, the user): compile, clippy and warning locations name
# files: a file only one change edited is that change's; a file none edited, the change(s) that edited its crate or a
# crate it uses. Failing tests name their crate: the change(s) that edited it or a crate it uses. Certain only when it
# comes to exactly one change (or changes of one worktree), the batch under it (landed, but its changes are in this tree) not among them; else
# (two candidates, T0, menu coverage, a timeout, a failure without a location) nothing, and the batch is split.
# Edits outside crates count as touching every crate, but docs (docs/AGENT_API.md: ops' coverage test).
# Prints "<key>…<TAB><why>".
blame() {   # blame <reason> <check log> <files the batches under it changed> <keys…>
  python3 - "$LD" "$@" <<'P'
import json, subprocess, sys, os, re
LD, reason, logf, depf, keys = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4], sys.argv[5:]
log = open(logf, errors='replace').read()
meta = json.loads(subprocess.run(['cargo', 'metadata', '--format-version', '1', '--no-deps'], capture_output=True, text=True).stdout)
root = meta['workspace_root']
pk = {p['name']: os.path.dirname(os.path.relpath(p['manifest_path'], root)) for p in meta['packages']}
deps = {p['name']: {d['name'] for d in p['dependencies'] if d['name'] in pk} for p in meta['packages']}
def closure(c):
    seen, todo = set(), [c]
    while todo:
        x = todo.pop()
        if x not in seen: seen.add(x); todo += deps.get(x, ())
    return seen
def crate_of(f): return next((n for n, d in sorted(pk.items(), key=lambda kv: -len(kv[1])) if f.startswith(d + '/')), None)
def files(path):
    try: ls = open(path).read().split('\n')
    except OSError: return []
    return [l for l in (ls[ls.index('--') + 1:] if '--' in ls else ls) if l]
def touches(fs, crates):
    for f in fs:
        c = crate_of(f)
        if c in crates or (f == 'docs/AGENT_API.md' and 'noshape-ops' in crates): return True
        if c is None and not f.startswith('docs/') and not f.endswith('.md'): return True
    return False
UNDER = '(under)'
jobs = {k: files(os.path.join(LD, 'jobs', k + '.job')) for k in keys}
jobs[UNDER] = files(depf)
cands, why = set(), ''
if reason.startswith(('cargo check', 'clippy', 'tests (build)')):
    locs = sorted(set(re.findall(r'^== at (\S+?):\d+:\d+$', log, re.M)))
    if not locs: sys.exit()
    for f in locs:
        t = {k for k, fs in jobs.items() if f in fs}
        if not t:
            c = crate_of(f)
            if c is None: sys.exit()
            t = {k for k, fs in jobs.items() if touches(fs, closure(c))}
        cands |= t
    why = f'the error{"s are" if len(locs) > 1 else " is"} in {", ".join(locs[:3])}{" …" if len(locs) > 3 else ""}'
elif reason == 'tests':
    failing = sorted(set(re.findall(r'^== failing `-p ([\w-]+)', log, re.M)))
    if not failing or any(p not in pk for p in failing): sys.exit()
    for p in failing: cands |= {k for k, fs in jobs.items() if touches(fs, closure(p))}
    why = f'{", ".join(failing)} failed: no other change in the batch edited {"them" if len(failing) > 1 else "it"} or a crate {"they use" if len(failing) > 1 else "it uses"}'
# (One worktree's queued changes include its earlier ones: candidates all from one worktree are that agent's chain.)
def name(k):
    try: return next((l[5:] for l in open(os.path.join(LD, 'jobs', k + '.job')) if l.startswith('name=')), k)
    except OSError: return k
if cands and UNDER not in cands and len({name(k) for k in cands}) == 1:
    print(f'{" ".join(sorted(cands))}\t{why}')
P
}

# Whether every commit of <ref2> not on main is in <ref> (by content, or rebased since: same author time and subject;
# land.sh rebases a worktree's commits each time it queues).
contains() {   # contains <ref> <ref2>
  local miss have c
  miss=$(git -C "$repo" rev-list --cherry-pick --right-only "$1...$2" "^refs/landq/$inst/target" 2>/dev/null) || return 1
  [ -n "$miss" ] || return 0
  have=$(git -C "$repo" log --format='%at %s' "refs/landq/$inst/target..$1")
  for c in $miss; do grep -qxF "$(git -C "$repo" log -1 --format='%at %s' "$c")" <<< "$have" || return 1; done
}

# The batch size that lands most per check at the recent red rate: ~1/p (at least 2), from the last 30 first checks
# (not halves or re-checks); 99 with fewer than 8 of them.
best_batch() {
  tail -n 60 "$LD/history.tsv" 2>/dev/null | awk -F'\t' '$4 == "batch" && $6 != "retest" && $6 != "result" { k[++n] = $5 " " ($6 == "blamed" || $6 == "split" || $6 == "failed") }
    END { if (n > 30) { o = n - 30 } ; for (i = o + 1; i <= n; i++) { split(k[i], a, " "); m++; s += a[1]; r += a[2] }
          if (m < 8 || r == 0) { print 99; exit } p = 1 - (1 - r / m) ^ (m / s); b = int(1 / p + 0.5); print (b < 2 ? 2 : b) }'
}

# Whether the batch under this one (if any) has ended without landing.
dep_failed() {
  [ -n "$1" ] || return 1
  if [ -e "$LD/outcome/$1" ]; then [ "$(cat "$LD/outcome/$1")" != landed ]; else [ ! -d "$LD/run/$1" ]; fi
}

# Whether a batch can be stacked on (by its run dir name): it and every batch under it are checking or green, or
# landed. Halves of red batches too: everything goes in one train (11-01: halves checked beside it landed first and
# sent the train's batches back to be checked again); one that turns red stops what is on it within seconds.
can_land() {
  local b=$1
  while [ -n "$b" ]; do
    [ -e "$LD/outcome/$b" ] && { [ "$(cat "$LD/outcome/$b")" = landed ]; return; }
    [ -s "$LD/run/$b/stack" ] || return 1
    b=$(cat "$LD/run/$b/dep" 2>/dev/null)
  done
}

# A batch, on a slot: stack, check, push or split.
run_batch() {
  local slot=$1 wt key keys=() applied=() conflict files other before commits c tip i k rc reason="" res moved hit t0 s
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
  # A change whose worktree has queued a later one that contains it (still queued, or being checked) goes with that
  # one: one check, its result (11-01: older changes of re-checked batches failed as conflicts with their own later
  # versions, rebased since).
  local nk k2 left=()
  for key in "${keys[@]}"; do
    left+=("$key"); [ -f "$LD/jobs/$key.job" ] && [ ! -e "$LD/jobs/$key.solo" ] || continue
    nk=""; for k2 in $(ls "$LD/jobs" | sed -n 's/\.job$//p' | sort); do [[ "$k2" > "$key" ]] && [ "$(field name "$k2")" = "$(field name "$key")" ] && nk=$k2; done
    [ -n "$nk" ] || continue
    git -C "$repo" fetch -q origin "+refs/heads/$(field ref "$nk"):refs/landq/$inst/job/$nk" 2>/dev/null || continue
    contains "refs/landq/$inst/job/$nk" "refs/landq/$inst/job/$key" || continue
    { echo "$key"; cat "$LD/jobs/$key.carry" 2>/dev/null; } >> "$LD/jobs/$nk.carry"; rm -f "$LD/jobs/$key.carry"
    log "$bid: $(field name "$key") ${key##*-} goes with its later ${nk##*-}"; unset 'left[-1]'
  done
  keys=("${left[@]}")
  # A change queued before an earlier one of its worktree failed, and containing it, fails with it unchecked (it was
  # being checked, or on a batch being checked, when that one failed; supersede() above takes the waiting ones).
  local fk fs ft
  for key in "${keys[@]}"; do
    [ -f "$LD/jobs/$key.job" ] && [ -s "$LD/failed/$(field name "$key")" ] || continue
    while read -r fk fs ft; do
      [ "$fk" != "$key" ] && [ "$ft" -gt "$(field queued "$key")" ] || continue
      git -C "$repo" cat-file -e "$fs^{commit}" 2>/dev/null || continue
      if contains "refs/landq/$inst/job/$key" "$fs"; then
        : > "$d/detail"; result "$key" FAILED "superseded: contains failed ${fs:0:8} (fix it on top, then land.sh)"; break
      fi
    done < "$LD/failed/$(field name "$key")"
  done
  # A carried change must be in the one carrying it (its commits, by content), else it is queued on its own again.
  local o keep
  for key in "${keys[@]}"; do
    [ -s "$LD/jobs/$key.carry" ] || continue
    keep=()
    for o in $(cat "$LD/jobs/$key.carry"); do
      if git -C "$repo" fetch -q origin "+refs/heads/$(field ref "$o"):refs/landq/$inst/job/$o" 2>/dev/null \
          && contains "refs/landq/$inst/job/$key" "refs/landq/$inst/job/$o"; then keep+=("$o")
      else : > "$LD/jobs/$o.solo"; : > "$LD/pending/$o"; log "$bid: $(field name "$o") ${o##*-} isn't in ${key##*-}: queued on its own again"; fi
    done
    if [ ${#keep[@]} -gt 0 ]; then printf '%s\n' "${keep[@]}" > "$LD/jobs/$key.carry"; else rm -f "$LD/jobs/$key.carry"; fi
  done
  # (A slot checkout deleted by hand stays registered: prune it, then add it again.)
  [ -d "$wt" ] || { git -C "$repo" worktree prune; git -C "$repo" worktree add -q --detach "$wt" "refs/landq/$inst/target"; }
  # (A slot's checkout may be left mid cherry-pick or rebase by a killed batch.)
  cd "$wt" || { log "$bid: no checkout $wt"; sleep 30; front resume "${keys[@]}"; return; }
  { git cherry-pick --quit; git rebase --quit; } 2>/dev/null
  # Stacked on the batch being checked before it, if any, not on main (11-01: batches side by side on main all edit
  # crates/ops, so each one that landed sent the others back to be checked again: half the checks were thrown away).
  # It lands right after that one, as checked; if that one doesn't land, this one goes back to the front.
  # Only on a batch that can still land: not red, and every batch under it the same (11-01: stacking on doomed
  # batches sent them round as re-checks for minutes). A red batch drops its stack file at once.
  exec 6>"$LD/stack.lock"; flock 6
  local dep="" r start="refs/landq/$inst/target"
  for r in $(ls -t "$LD"/run/*/stack 2>/dev/null); do
    r=$(basename "$(dirname "$r")"); [ "$LD/run/$r" = "$d" ] && continue
    [ -e "$LD/outcome/$r" ] && continue
    can_land "$r" && { dep=$r; start=$(cat "$LD/run/$r/stack"); break; }
  done
  # (On main: as it is now, not as fetched before the lock; a batch may have just landed.)
  [ -n "$dep" ] || git -C "$repo" fetch -q origin "+refs/heads/$TARGET:refs/landq/$inst/target"
  if [ -n "$dep" ]; then cat "$LD/run/$dep/changed" "$LD/run/$dep/dep.changed" > "$d/dep.changed" 2>/dev/null; else : > "$d/dep.changed"; fi
  git checkout -q -f --detach "$start" && git clean -fdq || { exec 6>&-; log "$bid: checkout failed"; sleep 30; front resume "${keys[@]}"; return; }
  base=$(git rev-parse HEAD); : > "$d/changed"; : > "$d/counts"; local pkgs=""
  for key in "${keys[@]}"; do
    [ -f "$LD/jobs/$key.job" ] || continue
    local jref="refs/landq/$inst/job/$key"
    git rev-parse -q --verify "$jref" > /dev/null || continue
    # Its commits not yet on the stack (by content: a change already landed by --wait or with an earlier job drops out).
    commits=$(git rev-list --reverse --cherry-pick --right-only HEAD..."$jref")
    # None: on main already (landed), or all in an earlier change of this batch (it lands or fails with them).
    if [ -z "$commits" ] && [ "$(git rev-parse HEAD)" = "$base" ] && [ -z "$dep" ]; then : > "$d/detail"; result "$key" LANDED "$base"; continue; fi
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
      elif [ -n "$dep" ]; then reason="rebase conflict with $TARGET or a change being checked before it (in $(echo $files)): rebase on origin/$TARGET once that lands, then re-run land.sh"
      else reason="rebase conflict with $TARGET (in $(echo $files)): rebase on origin/$TARGET by hand, then re-run land.sh"; fi
      result "$key" FAILED "$reason"; continue
    fi
    applied+=("$key"); echo "$key $(git rev-list --count "$before"..HEAD)" >> "$d/counts"
    sed -n '/^--$/,$p' "$LD/jobs/$key.job" | tail -n +2 >> "$d/changed"; pkgs="$pkgs $(field pkgs "$key")"
  done
  n=${#applied[@]}; local t_stack=$(( $(date +%s) - t0 )); reason=""
  # (Nothing applied: no stack, so nothing stacks on this batch.)
  [ $n -gt 0 ] || { exec 6>&-; res_word=landed; return; }
  git rev-parse HEAD > "$d/stack"; echo "$dep" > "$d/dep"; git update-ref "refs/landq/$inst/stack/$bid" HEAD; exec 6>&-
  pkgs=$(echo $pkgs | tr ' ' '\n' | sort -u | tr '\n' ' '); sort -u -o "$d/changed" "$d/changed"
  # Nothing beyond the batch under it (its changes came in with later ones of their worktrees, 11-01): no check of
  # the same tree again; it lands or goes back with that one.
  if [ "$(git rev-parse HEAD)" = "$base" ] && [ -n "$dep" ]; then
    log "$bid: $kind of $n adds nothing to $dep under it: lands with it, unchecked"; echo "waiting for $dep" > "$d/state"
    while [ ! -e "$LD/outcome/$dep" ] && [ -d "$LD/run/$dep" ]; do sleep 3; done
    if [ "$(cat "$LD/outcome/$dep" 2>/dev/null)" = landed ]; then
      res_word=landed; for k in "${applied[@]}"; do : > "$d/detail"; result "$k" LANDED "$base"; done
    else
      rm -f "$d/stack"; front retest "${applied[@]}"; res_word=retest
    fi
    return
  fi
  echo "checking" > "$d/state"; log "$bid: $kind of $n on ${base:0:8}${dep:+ (on $dep)} in slot $slot: $(for k in "${applied[@]}"; do printf '%s ' "$(field name "$k"):${k##*-}"; done)"
  prune_land_target "$CARGO_TARGET_DIR" > /dev/null
  : > "$d/times"; s=$(date +%s)
  # (In its own process group, stopped as soon as the batch under it ends without landing: its result can't be used.)
  LAND_TIMES="$d/times" setsid timeout 150m ${LAND_CHECKS:-tools/tiers/land-checks.sh} "$pkgs" "$d/changed" > "$d/check.log" 2>&1 &
  local cpid=$!
  while kill -0 "$cpid" 2>/dev/null; do
    if dep_failed "$dep"; then kill -TERM -- "-$cpid" 2>/dev/null; log "$bid: check stopped: $dep under it didn't land"; break; fi
    sleep 5
  done
  wait "$cpid"; rc=$?
  local t_check=$(( $(date +%s) - s )) t_push=0 depres=landed
  [ $rc -eq 0 ] || rm -f "$d/stack"   # (red: nothing stacks on it any more)
  # Red, and the error is certainly its own (not the batch under it): no waiting for that one (11-01: a409fa87 waited
  # 24 min behind a batch of 4 that then failed, and was checked again).
  if [ $rc -ne 0 ] && [ -n "$dep" ] && [ -n "$(blame "$(sed -n 's/^LAND FAILED: //p' "$d/check.log" | tail -1)" "$d/check.log" "$d/dep.changed" "${applied[@]}" 2>>"$LD/daemon.log")" ]; then
    log "$bid: red, from the error its own: not waiting for $dep"; dep=""
  fi
  if [ -n "$dep" ]; then
    # Green, it can land only once the batch under it has (its commits are in this stack); red, it is only to blame
    # if that one landed (else its bad change may be what failed here).
    echo "waiting for $dep" > "$d/state"
    while [ ! -e "$LD/outcome/$dep" ] && [ -d "$LD/run/$dep" ]; do sleep 3; done
    depres=$(cat "$LD/outcome/$dep" 2>/dev/null); depres=${depres:-gone}
    if [ "$depres" != landed ]; then
      rm -f "$d/stack"; reason="$dep under it: $depres"; log "$bid: again from the front ($dep, under it, didn't land: $depres)"
      front retest "${applied[@]}"; res_word=retest
    fi
  fi
  if [ "$depres" != landed ]; then
    :
  elif [ $rc -eq 0 ]; then
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
      # (Only commits that weren't in the checked stack: the batch under it, landed, was checked with this one.)
      moved=$(for c in $(git rev-list --cherry-pick --left-only "refs/landq/$inst/target"...HEAD); do git show --name-only --format= "$c"; done | sort -u)
      [ -n "$moved" ] || sleep 10   # (rejected though main didn't move: GitHub or the relay; try again shortly)
      hit=$(overlap "$moved" "$(cat "$d/changed")")
      if [ -n "$hit" ] || ! git rebase -q "refs/landq/$inst/target" > /dev/null 2>&1; then
        git rebase --abort 2>/dev/null
        reason="${hit:-rebase conflict after checking}"; [ -z "$hit" ] || reason="$TARGET changed $hit while checking"
        log "$bid: again from the front ($reason)"
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
        i=$(( i + cnt )); tip=${c[$((i - 1))]:-}; [ $i -gt 0 ] || tip=$(git rev-parse HEAD); [ ${#c[@]} -eq "$(awk '{s+=$2} END {print s}' "$d/counts")" ] || tip=$(git rev-parse HEAD)
        : > "$d/detail"; result "$k" LANDED "$tip"
      done < "$d/counts"
    fi
  else
    reason=$(sed -n 's/^LAND FAILED: //p' "$d/check.log" | tail -1); [ $rc -eq 124 ] && reason="timeout (checks over 150 min)"
    reason=${reason:-"checks exited $rc"}
    local bl=""; [ $n -gt 1 ] && bl=$(blame "$reason" "$d/check.log" "$d/dep.changed" "${applied[@]}" 2>>"$LD/daemon.log")
    if [ $n -eq 1 ]; then
      res_word=failed
      grep -vE '^(== |CHECKS PASSED)' "$d/check.log" | grep -v '^LAND FAILED' | tail -60 > "$d/detail"
      result "${applied[0]}" FAILED "$reason"
    elif [ -n "$bl" ]; then
      # Blamed from the error: failed at once; the others are checked again without it (a second failure may hide
      # behind the first; nothing unchecked goes to main).
      local culprits=" ${bl%%$'\t'*} " why=${bl#*$'\t'} rest=() who
      for k in "${applied[@]}"; do [[ "$culprits" == *" $k "* ]] || rest+=("$k"); done
      res_word=blamed; who=$(for k in $culprits; do printf '%s ' "$(field name "$k") ${k##*-}"; done)
      { echo "(found in a batch of $n from the error: $why)"; grep -vE '^(== |CHECKS PASSED)' "$d/check.log" | grep -v '^LAND FAILED' | tail -60; } > "$d/detail"
      for k in $culprits; do result "$k" FAILED "$reason"; done
      log "$bid: red ($reason): blamed ${who% } ($why); the other ${#rest[@]} again without it"
      [ ${#rest[@]} -eq 0 ] || front rest "${rest[@]}"; reason="$reason; blamed ${who% }: $why"
    else
      # Red: halves go first (stacked like any batch, so in the train); the one at fault is the half that stays red, down to one.
      res_word=split; local h=$(( (n + 1) / 2 ))
      log "$bid: red ($reason): no single change from the error: splitting $n into $h + $(( n - h ))"
      front split "${applied[@]:0:$h}"; front split "${applied[@]:$h}"
    fi
  fi
  # Timings of this check (docs/TESTS.md "Landing"): queue wait = queued → claimed, the longest in the batch.
  local qw=0 q; for k in "${applied[@]}"; do q=$(( t_claim - $(field queued "$k") )); [ $q -gt $qw ] && qw=$q; done
  [ -s "$LD/history.tsv" ] || printf 'end\thost\tbatch\tkind\tsize\tresult\tqueue_wait\tslot_wait\tstack\tcheck_total\tcheck\tclippy\ttests_build\ttests_run\tt0\tmenu\tpush\treason\tcrates\n' > "$LD/history.tsv"
  tm() { awk -v p="$1" '$1 == p {print $2}' "$d/times"; }
  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$(date -u '+%F %T')" "$(hostname -s)" "$bid" "$kind" "$n" \
    "$res_word" "$qw" "$SLOT_WAITED" "$t_stack" "$t_check" "$(tm check)" "$(tm clippy)" "$(tm tests_build)" "$(tm tests_run)" "$(tm t0)" "$(tm menu)" \
    "$t_push" "$reason" "$(echo $pkgs | wc -w)" >> "$LD/history.tsv"
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
      # One place per worktree: its newest queued change carries its older ones (they are in it; checked at stacking),
      # so a run of lands is checked once (11-01: 39 queued were 5 worktrees' runs). Batch size grows with the
      # backlog: 4 worktrees, 8 above 12 queued changes, 12 above 24 (~3 % of changes fail, and blame makes red batches cheap).
      local -A newest=(); local order=() k nm max ks=() npend
      npend=$(ls "$LD/pending" | wc -l)
      for k in $(ls "$LD/pending"); do
        nm=$(field name "$k"); [ -e "$LD/jobs/$k.solo" ] && nm="#$k"
        if [ -n "${newest[$nm]:-}" ]; then
          { echo "${newest[$nm]}"; cat "$LD/jobs/${newest[$nm]}.carry" 2>/dev/null; } >> "$LD/jobs/$k.carry"
          rm -f "$LD/jobs/${newest[$nm]}.carry" "$LD/pending/${newest[$nm]}"
        else order+=("$nm"); fi
        newest[$nm]=$k
      done
      max=$MAXB; [ "$npend" -gt 12 ] && max=$(( MAXB * 2 )); [ "$npend" -gt 24 ] && max=$(( MAXB * 3 ))
      # …and never above ~1/p, p a stack's chance to be red in the recent first checks (11-01 afternoon ~30 %, not
      # the 3 % per change of the model: a worktree's run is one stack; most batches of 4-5 went red).
      local b; b=$(best_batch); [ "$b" -lt "$max" ] && max=$b
      for nm in "${order[@]:0:$max}"; do ks+=("${newest[$nm]}"); done
      if [ ${#ks[@]} -gt 0 ]; then
        bid="$(date -u +%m%d-%H%M%S)-$BASHPID"; mkdir -p "$LD/run/$bid"; echo "$BASHPID" > "$LD/run/$bid/pid"
        { echo kind=batch; printf '%s\n' "${ks[@]}"; } > "$LD/run/$bid/jobs"
        for k in "${ks[@]}"; do rm -f "$LD/pending/$k"; done
      fi
    fi
  } 5>"$LD/claim.lock"
  rm -f "$tok"
  [ -n "$bid" ] || return 0
  t_claim=$(date +%s); res_word=retest
  run_batch "$slot"
  # What the batches stacked on this one wait for: landed, or not (they go back to the front).
  echo "$res_word" > "$LD/outcome/$bid"; git -C "$repo" update-ref -d "refs/landq/$inst/stack/$bid" 2>/dev/null
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
  recover; waiting=0; find "$LD/outcome" "$LD/failed" -type f -mmin +1440 -delete 2>/dev/null
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
  # A waiter (in the slots' queue) per thing to claim: each front batch, and what is queued; up to the free slots.
  units=$(( $(ls "$LD"/splits/*.batch 2>/dev/null | wc -l) + $( [ -n "$(ls "$LD/pending")" ] && echo 1 || echo 0) ))
  if [ $waiting -lt $units ] && [ $(( busy + waiting )) -lt "$LAND_SLOTS" ]; then
    tok="$LD/waiting.$(date +%s%N)"; : > "$tok"
    ( exec 9>&-; worker "$tok" ) >> "$LD/daemon.log" 2>&1 &
    echo $! > "$tok"
  fi
  sleep 2
done
log "stopped ($LD/stop)"; rm -f "$LD/stop"
