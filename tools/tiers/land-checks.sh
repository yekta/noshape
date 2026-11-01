#!/bin/bash
# land.sh's checks for one change or a batch of queued changes, run on the land server (tools/tiers/land-daemon.sh,
# land-remote.sh) or here: `cargo check` over the workspace (type errors and warnings anywhere fail), clippy on the
# crates the change edits, the full tests of the given crates, T0 when the app / agent API is affected, ops' menu
# coverage when a UI crate changed. LAND_TIMES=<file>: each phase's seconds are appended to it.
# Usage: land-checks.sh "<crates>" <file listing the changed paths>
set -uo pipefail
cd "$(git rev-parse --show-toplevel)"
pkgs=$1; changed=$(cat "$2")
fail() { echo "LAND FAILED: $*"; exit 1; }
phase() { [ -z "${LAND_TIMES:-}" ] || echo "$1 $(( $(date +%s) - $2 ))" >> "$LAND_TIMES"; }
# The app reads and writes its settings (toolbars, graphics, shortcuts, recent files) in NOSHAPE_CONFIG_DIR, else
# ~/.config/noshape: a fresh dir per run, so parallel runs (the watcher, two land checks on vibe3) never share them (10-29).
[ -n "${NOSHAPE_CONFIG_DIR:-}" ] || { NOSHAPE_CONFIG_DIR=$(mktemp -d -t noshape-config-XXXXXX); export NOSHAPE_CONFIG_DIR; }
args=$(for p in $pkgs; do printf -- '-p %s ' "$p"; done)
# Decide on cargo's exit code (with pipefail a grep can't decide it), then show the relevant lines.
show() { echo "$1" | grep -vE '^warning: [a-z0-9_-]+@' | grep -E "$2" -A"$3" | head -60; }
# All of a failure's error locations and failing test targets, in full: the land daemon blames the change at fault
# from them (tools/tiers/land-daemon.sh).
at() { echo "$1" | grep -vE '^warning: [a-z0-9_-]+@' | sed -n 's/^ *--> \([^ ]*\)$/== at \1/p' | sort -u | head -100; }
targets() { echo "$1" | grep -oE 'rerun pass `-p [^`]*`' | sort -u | sed 's/^rerun pass /== failing /'; }
# The crates the change edits ("all" when a manifest changed).
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
# Clippy over the whole workspace re-linted every crate downstream of the change (11-01): now the workspace is only
# type-checked (rustc's warnings still fail) and clippy lints the edited crates, their dependencies checked once.
if [ "$edited" = all ]; then
  echo "== clippy (workspace: a manifest changed)"; s=$(date +%s); out=$(cargo clippy --workspace --all-targets -- -D warnings 2>&1); rc=$?; phase clippy "$s"
  [ $rc -eq 0 ] || { show "$out" '^(error|warning)' 6; at "$out"; fail "clippy"; }
else
  echo "== check (workspace)"; s=$(date +%s); out=$(cargo check --workspace --all-targets 2>&1); rc=$?; phase check "$s"
  warns=$(echo "$out" | grep -vE '^warning: [a-z0-9_-]+@' | grep -cE '^warning(\[|:)')
  [ $rc -eq 0 ] && [ "$warns" = 0 ] || { show "$out" '^(error|warning)' 6; at "$out"; fail "cargo check (errors or warnings)"; }
  if [ -n "$edited" ]; then
    echo "== clippy ($edited)"; s=$(date +%s)
    out=$(cargo clippy --no-deps $(for p in $edited; do printf -- '-p %s ' "$p"; done) --all-targets -- -D warnings 2>&1); rc=$?; phase clippy "$s"
    [ $rc -eq 0 ] || { show "$out" '^(error|warning)' 6; at "$out"; fail "clippy"; }
  fi
fi
echo "== tests (build)"; s=$(date +%s); out=$(cargo test $args --no-run 2>&1); rc=$?; phase tests_build "$s"
[ $rc -eq 0 ] || { show "$out" '^error|Caused by' 6; at "$out"; fail "tests (build)"; }
echo "== tests"; s=$(date +%s); out=$(cargo test $args --no-fail-fast 2>&1); rc=$?; phase tests_run "$s"
[ $rc -eq 0 ] || { show "$out" 'FAILED|panicked|^error|SIGSEGV|signal:|Caused by' 2; targets "$out"; fail "tests"; }
# (Whole names: grep -w took noshape-brep for noshape, and every land ran T0 (11-01).)
if echo " $pkgs " | grep -qE ' (noshape|noshape-ops|noshape-cli) '; then
  echo "== T0"; s=$(date +%s); tools/tiers/t0.sh > /tmp/land-t0-$$.log 2>&1; rc=$?; phase t0 "$s"
  [ $rc -eq 0 ] || { tail -20 /tmp/land-t0-$$.log; fail "T0"; }
fi
# Menu labels live in the UI crates, but the test that every one has an agent API op (ops coverage) doesn't depend
# on them, so the crate list above misses it (10-28: 19f2d38a's "Move to origin" turned main red).
if ! echo " $pkgs " | grep -q ' noshape-ops ' && echo "$changed" | grep -qE '^crates/(app|[a-z-]+-ui)/.*\.rs$'; then
  echo "== menu coverage"; s=$(date +%s); out=$(cargo test -p noshape-ops --test all coverage:: 2>&1); rc=$?; phase menu "$s"
  [ $rc -eq 0 ] || { echo "$out" | grep -E 'FAILED|panicked' -A8 | head -30; fail "menu coverage (add the label to crates/ops/src/coverage.rs MENUS)"; }
fi
echo "CHECKS PASSED"
