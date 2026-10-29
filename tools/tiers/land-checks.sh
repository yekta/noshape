#!/bin/bash
# land.sh's checks for one change, run here or on the land server (tools/tiers/land-remote.sh): clippy and the full tests
# of the given crates, T0 when the app / agent API is affected, ops' menu coverage when a UI crate changed.
# Usage: land-checks.sh "<crates>" <file listing the changed paths>
set -uo pipefail
cd "$(git rev-parse --show-toplevel)"
pkgs=$1; changed=$(cat "$2")
fail() { echo "LAND FAILED: $*"; exit 1; }
# The app reads and writes its settings (toolbars, graphics, shortcuts, recent files) in NOSHAPE_CONFIG_DIR, else
# ~/.config/noshape: a fresh dir per run, so parallel runs (the watcher, two land checks on vibe3) never share them (10-29).
[ -n "${NOSHAPE_CONFIG_DIR:-}" ] || { NOSHAPE_CONFIG_DIR=$(mktemp -d -t noshape-config-XXXXXX); export NOSHAPE_CONFIG_DIR; }
args=$(for p in $pkgs; do printf -- '-p %s ' "$p"; done)
# Decide on cargo's exit code (with pipefail a grep can't decide it), then show the relevant lines.
echo "== clippy"; out=$(cargo clippy $args --all-targets -- -D warnings 2>&1); rc=$?
[ $rc -eq 0 ] || { echo "$out" | grep -vE '^warning: [a-z0-9_-]+@' | grep -E '^(error|warning)' -A6 | head -60; fail "clippy"; }
echo "== tests"; out=$(cargo test $args --no-fail-fast 2>&1); rc=$?
[ $rc -eq 0 ] || { echo "$out" | grep -vE '^warning: [a-z0-9_-]+@' | grep -E 'FAILED|panicked|^error|SIGSEGV|signal:|Caused by' -A2 | head -60; fail "tests"; }
if echo "$pkgs" | grep -qwE 'noshape|noshape-ops|noshape-cli'; then echo "== T0"; tools/tiers/t0.sh > /tmp/land-t0-$$.log 2>&1 || { tail -20 /tmp/land-t0-$$.log; fail "T0"; }; fi
# Menu labels live in the UI crates, but the test that every one has an agent API op (ops coverage) doesn't depend
# on them, so the crate list above misses it (10-28: 19f2d38a's "Move to origin" turned main red).
if ! echo "$pkgs" | grep -qw 'noshape-ops' && echo "$changed" | grep -qE '^crates/(app|[a-z-]+-ui)/.*\.rs$'; then
  echo "== menu coverage"; out=$(cargo test -p noshape-ops --test all coverage:: 2>&1) || { echo "$out" | grep -E 'FAILED|panicked' -A8 | head -30; fail "menu coverage (add the label to crates/ops/src/coverage.rs MENUS)"; }
fi
echo "CHECKS PASSED"
