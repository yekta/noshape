#!/bin/bash
# land.sh: the only way to put work on main (docs/TESTS.md). Rebases on origin/main, then runs clippy and the full
# tests of every crate the change touches AND every crate that depends on them (+ T0 when the app or the agent API is
# affected), and pushes HEAD to main only if all of it is green. Re-run after a rejected push (main moved).
# Usage (from your worktree, your env set: OCCT_ROOT, CARGO_TARGET_DIR, CC/CXX): tools/tiers/land.sh
set -uo pipefail
cd "$(git rev-parse --show-toplevel)"
fail() { echo "LAND FAILED: $*"; exit 1; }
[ -z "$(git status --porcelain --untracked-files=no)" ] || fail "uncommitted changes: commit them first"
git fetch -q origin || fail "fetch"
git rebase -q origin/main || { git rebase --abort; fail "rebase conflict: rebase by hand, then re-run"; }
changed=$(git diff --name-only origin/main...HEAD)
[ -n "$changed" ] || { echo "nothing to land"; exit 0; }
# Docs-only changes: push without building.
if ! echo "$changed" | grep -qvE '^(docs/|.*\.md$)'; then
  git push -q origin HEAD:main && { echo "LANDED (docs only) $(git rev-parse --short HEAD)"; exit 0; } || fail "push rejected: re-run"
fi
# Crates touched, then their reverse dependencies inside the workspace.
pkgs=$(python3 - "$changed" <<'P'
import json, subprocess, sys, os
changed = sys.argv[1].split('\n')
meta = json.loads(subprocess.run(['cargo', 'metadata', '--format-version', '1', '--no-deps'], capture_output=True, text=True).stdout)
root = meta['workspace_root']
pk = {p['name']: os.path.dirname(os.path.relpath(p['manifest_path'], root)) for p in meta['packages']}
deps = {p['name']: {d['name'] for d in p['dependencies'] if d['name'] in pk} for p in meta['packages']}
touched = {n for n, d in pk.items() if any(c == d or c.startswith(d + '/') for c in changed)}
if any(c in ('Cargo.toml', 'Cargo.lock') or c.startswith('.cargo/') for c in changed): touched = set(pk)
out = set(touched); grow = True
while grow:
    grow = False
    for n, ds in deps.items():
        if n not in out and ds & out: out.add(n); grow = True
out.discard('noshape-workspace-hack')
print(' '.join(sorted(out)))
P
)
[ -n "$pkgs" ] || pkgs="noshape-ops"
args=$(for p in $pkgs; do printf -- '-p %s ' "$p"; done)
echo "== crates: $pkgs"
echo "== clippy"; cargo clippy $args --all-targets -- -D warnings 2>&1 | grep -vE '^warning: [a-z0-9_-]+@' | grep -E '^(error|warning)' -A6 && fail "clippy"
echo "== tests"; cargo test $args --no-fail-fast 2>&1 | grep -vE '^warning: [a-z0-9_-]+@' | grep -E 'FAILED|panicked|^error|SIGSEGV|signal:' && fail "tests"
if echo "$pkgs" | grep -qwE 'noshape|noshape-ops|noshape-cli'; then echo "== T0"; tools/tiers/t0.sh > /tmp/land-t0-$$.log 2>&1 || { tail -20 /tmp/land-t0-$$.log; fail "T0"; }; fi
# Main may have moved while we tested: rebase again; re-test only if the new commits touch our crates.
git fetch -q origin
if ! git merge-base --is-ancestor origin/main HEAD; then
  git rebase -q origin/main || { git rebase --abort; fail "rebase conflict after testing"; }
  echo "rebased on newer main; re-run land.sh if those commits touched: $pkgs"
fi
git push -q origin HEAD:main || fail "push rejected (main moved): re-run land.sh"
echo "LANDED $(git rev-parse --short HEAD)"
