#!/bin/bash
# Trunk-based main watcher (replaces the merge queue, 10-25). Agents push small, clippy-checked commits straight to main.
# This loop tests the newest main in /root/noshape-lead (target-lead): regenerate docs/AGENT_API.md (push if changed),
# then app build + workspace tests + clippy. Green -> move tag last-green (pushed) + /root/green/noshape (QA/release binary).
# Red -> /tmp/lead/main-status lists the failures and the commits since last-green (subjects start "<agent>: ").
set -u
export OCCT_ROOT=/root/occt CARGO_TARGET_DIR=/root/target-lead CARGO_INCREMENTAL=0  # fresh build per commit: incremental only costs disk here
export CC=gcc-13 CXX=g++-13  # CI (ubuntu-24.04) compiles our C++ with GCC 13; GCC 15 here accepts code CI rejects
REPO=/root/noshape; WT=/root/noshape-lead; ST=/tmp/lead/main-status; LOG=/tmp/lead/watch.log
G="git -c user.name=yekta -c user.email=ngyekta@gmail.com"
log() { echo "$(date -u +%H:%M:%S) $*" >> $LOG; }
quick() { # $1 = previous tested sha: run only the test suites of the crates the new commits touched (fast red)
  cd $WT; [ -z "$1" ] && return
  local pk="" d n
  for d in $(git diff --name-only "$1" HEAD -- crates | cut -d/ -f2 | sort -u); do
    n=$(grep -m1 '^name' crates/$d/Cargo.toml 2>/dev/null | cut -d'"' -f2); [ -n "$n" ] && [ "$n" != noshape ] && pk="$pk -p $n"
  done
  [ -z "$pk" ] && return
  local t0=$SECONDS t; t=$(nice -n -5 cargo test $pk 2>&1)
  log "  quick tests ($pk) $((SECONDS-t0)) s"
  echo "$t" | grep -vE "^warning: [a-z0-9_-]+@[0-9.]+:" | grep -E "FAILED|panicked|^error|signal:|SIGSEGV|SIGABRT|stack overflow|Caused by|process didn.t exit" | head -14
}
check() {
  cd $WT; local t0=$SECONDS
  local b; b=$(nice -n -5 cargo build -p noshape 2>&1 | grep -E "^error" -A4 | head -8)
  [ -n "$b" ] && { echo "build: $b"; return; }
  log "  app build $((SECONDS-t0)) s"; t0=$SECONDS
  local t; t=$(nice -n -5 cargo test --workspace --exclude noshape 2>&1)
  echo "$t" | grep -q "could not execute process" && t=$(nice -n -5 cargo test --workspace --exclude noshape 2>&1)
  log "  workspace tests $((SECONDS-t0)) s"; t0=$SECONDS
  echo "$t" | grep -vE "^warning: [a-z0-9_-]+@[0-9.]+:" | grep -E "FAILED|panicked|^error|signal:|SIGSEGV|SIGABRT|stack overflow|Caused by|process didn.t exit|os error" | head -14
  # Build-script output ("warning: <crate>@<ver>: ..." from OCCT headers) isn't a lint; clippy's own errors are.
  nice -n -5 cargo clippy --locked --workspace --all-targets -- -D warnings 2>&1 | grep -vE "^warning: [a-z0-9_-]+@[0-9.]+:" | grep -E "^(error|warning)" -A3 | head -8
  log "  clippy $((SECONDS-t0)) s"
}
last=""
while true; do
  git -C $REPO fetch -q origin 2>/dev/null
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
    f=$(quick "$last"); [ -z "$f" ] && f=$(check)
    if [ -z "$f" ]; then
      git -C $REPO tag -f last-green "$m" >/dev/null && git -C $REPO push -q -f origin last-green 2>/dev/null
      cp $CARGO_TARGET_DIR/debug/noshape /root/green/noshape.new && mv /root/green/noshape.new /root/green/noshape && echo "$m" > /root/green/SHA
      echo "GREEN $(git -C $REPO rev-parse --short $m) at $(date -u +%H:%M)" > $ST; log "GREEN $m"
    else
      { echo "RED $(git -C $REPO rev-parse --short $m) at $(date -u +%H:%M)"; echo "$f"; echo "--- commits since last-green:"; git -C $REPO log --format='%h %s' last-green..$m | cut -c1-120; } > $ST
      log "RED $m: $(echo "$f" | head -2 | tr '\n' ' ' | cut -c1-200)"
    fi
    last=$m
  fi
  sleep 60
done
