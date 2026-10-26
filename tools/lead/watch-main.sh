#!/bin/bash
# Main watcher (docs/PLAN.md, docs/TESTS.md; 10-26). Tests the newest main in /root/projects/noshape-wt/watch (target-lead) with
# T0 + T1 + clippy. Green -> tag last-green (pushed: CI runs T1 on all 3 platforms) and /root/green/noshape.
# Red -> /tmp/lead/main-status lists the failures and the commits since last-green; if exactly one commit landed since
# last-green, it is reverted at once (PLAN 2.4: no grace period; the author re-lands it fixed).
# Nightly (first loop after 01:00 UTC): T2 on last-green -> /tmp/lead/t2-status + /tmp/lead/t2-<date>.log. T2 never blocks.
set -u
export OCCT_ROOT=/root/occt CARGO_TARGET_DIR=/root/target-lead CARGO_INCREMENTAL=0
export CC=gcc-13 CXX=g++-13  # CI (ubuntu-24.04) compiles our C++ with GCC 13; GCC 15 here accepts code CI rejects
REPO=/root/projects/noshape; WT=/root/projects/noshape-wt/watch; ST=/tmp/lead/main-status; LOG=/tmp/lead/watch.log
G="git -c user.name=yekta -c user.email=ngyekta@gmail.com"
log() { echo "$(date -u +%H:%M:%S) $*" >> $LOG; }
fails() { grep -vE "^warning: [a-z0-9_-]+@[0-9.]+:" | grep -E "FAILED|panicked|^error|signal:|SIGSEGV|SIGABRT|stack overflow|Caused by|process didn.t exit" | head -14; }
check() {
  cd $WT; local t0=$SECONDS o
  o=$(nice -n -5 tools/tiers/t0.sh 2>&1) || { echo "T0:"; echo "$o" | fails; [ -z "$(echo "$o" | fails)" ] && echo "$o" | tail -5; return; }
  log "  T0 $((SECONDS-t0)) s"; t0=$SECONDS
  o=$(nice -n -5 tools/tiers/t1.sh --no-fail-fast 2>&1); local rc=$?
  log "  T1 $((SECONDS-t0)) s"; t0=$SECONDS
  [ $rc -ne 0 ] && { echo "T1:"; echo "$o" | fails; [ -z "$(echo "$o" | fails)" ] && echo "$o" | tail -5; }
  # Build-script output ("warning: <crate>@<ver>: ..." from OCCT headers) isn't a lint; clippy's own errors are.
  nice -n -5 cargo clippy --locked --workspace --all-targets -- -D warnings 2>&1 | grep -vE "^warning: [a-z0-9_-]+@[0-9.]+:" | grep -E "^(error|warning)" -A3 | head -8
  log "  clippy $((SECONDS-t0)) s"
}
nightly() { # T2 on last-green, once a day after 01:00 UTC
  local day; day=$(date -u +%F)
  [ "$(date -u +%H)" -ge 1 ] || return
  [ "$(cat /tmp/lead/t2-day 2>/dev/null)" = "$day" ] && return
  echo "$day" > /tmp/lead/t2-day
  cd $WT; git reset -q --hard; git clean -fdq; git checkout -q --detach last-green
  log "T2 nightly on $(git rev-parse --short HEAD)"
  local t0=$SECONDS o; o=$(nice -n 5 tools/tiers/t2.sh 2>&1)
  echo "$o" > /tmp/lead/t2-$day.log
  { echo "T2 $day on $(git rev-parse --short HEAD), $((SECONDS-t0)) s"; echo "$o" | grep -E "^test .* FAILED|test result" ; } > /tmp/lead/t2-status
  log "T2 done $((SECONDS-t0)) s: $(grep -c FAILED /tmp/lead/t2-status) failed"
}
last=""
while true; do
  git -C $REPO fetch -q origin --tags -f 2>/dev/null
  m=$(git -C $REPO rev-parse origin/main)
  if [ "$m" != "$last" ]; then
    cd $WT; git reset -q --hard; git clean -fdq; git checkout -q --detach "$m"
    # The agent API reference is generated: keep main's copy current without anyone having to.
    UPDATE_DOCS=1 nice -n -5 cargo test -q -p noshape-ops --test coverage >/dev/null 2>&1
    if ! git diff --quiet docs/AGENT_API.md; then
      git add docs/AGENT_API.md; $G commit -q -m "lead: regenerate docs/AGENT_API.md"
      for i in 1 2 3 4 5; do git push -q origin HEAD:main 2>/dev/null && { log "pushed AGENT_API.md regen"; break; }; git fetch -q origin; git rebase -q origin/main 2>/dev/null || { git rebase --abort; break; }; done
      git -C $REPO fetch -q origin; m=$(git -C $REPO rev-parse origin/main); git checkout -q --detach "$m"
    fi
    log "testing $(git log -1 --format='%h %s' $m | cut -c1-90)"
    f=$(check)
    if [ -z "$f" ]; then
      git -C $REPO tag -f last-green "$m" >/dev/null && git -C $REPO push -q -f origin last-green 2>/dev/null
      mkdir -p /root/green; cp $CARGO_TARGET_DIR/debug/noshape /root/green/noshape.new && mv /root/green/noshape.new /root/green/noshape && echo "$m" > /root/green/SHA
      echo "GREEN $(git -C $REPO rev-parse --short $m) at $(date -u +%H:%M)" > $ST; log "GREEN $m"
    else
      since=$(git -C $REPO log --format='%h %s' last-green..$m | grep -v "^[0-9a-f]* lead: regenerate docs/AGENT_API.md")
      { echo "RED $(git -C $REPO rev-parse --short $m) at $(date -u +%H:%M)"; echo "$f"; echo "--- commits since last-green:"; echo "$since" | cut -c1-120; } > $ST
      log "RED $m: $(echo "$f" | head -3 | tr '\n' ' ' | cut -c1-200)"
      # One culprit: revert it at once (on top of the newest main).
      if [ "$(echo "$since" | grep -c .)" = 1 ] && [ "$(git -C $REPO rev-list --count --merges last-green..$m)" = 0 ]; then
        c=$(echo "$since" | cut -d' ' -f1)
        git fetch -q origin; git checkout -q --detach origin/main
        if $G revert --no-edit "$c" >/dev/null 2>&1 && git commit -q --amend -m "lead: revert $(git log -1 --format=%s $c | cut -c1-80)" -m "Reverted by the watcher: main red on T0/T1 ($(echo "$f" | grep -m1 -E 'FAILED|panicked|error' | cut -c1-150)). Re-land it fixed." && git push -q origin HEAD:main; then
          echo "REVERTED $c" >> $ST; log "REVERTED $c"
        else
          git revert --abort 2>/dev/null; echo "REVERT FAILED for $c: lead must revert by hand" >> $ST; log "revert of $c failed"
        fi
      fi
    fi
    last=$m
  fi
  nightly
  sleep 60
done
