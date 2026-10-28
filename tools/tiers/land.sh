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
echo "== crates: $pkgs"
# The checks (tools/tiers/land-checks.sh) run on the land server, vibe3, so agents' full test runs don't load this
# machine (10-28): the commit goes to a temporary land/ branch, vibe3 checks it in this worktree's own worktree and
# target dir there. NOSHAPE_LAND_LOCAL=1, or vibe3 unreachable: here, as before.
name=$(basename "$PWD"); cf=$(mktemp); echo "$changed" > "$cf"
if [ "${NOSHAPE_LAND_LOCAL:-0}" != 1 ] && ssh -o BatchMode=yes -o ConnectTimeout=5 vibe3 true 2>/dev/null; then
  branch="land/$name-$(git rev-parse --short HEAD)"
  git push -q -f origin "HEAD:refs/heads/$branch" 2>/dev/null || fail "push to $branch"
  ssh -o BatchMode=yes vibe3 "mkdir -p ~/land && cd ~/projects/noshape && git fetch -q origin $branch && git show FETCH_HEAD:tools/tiers/land-remote.sh > ~/land/remote-$name.sh && bash ~/land/remote-$name.sh '$name' '$branch' '$pkgs'" < "$cf"
  rc=$?; git push -q origin --delete "$branch" 2>/dev/null
else
  echo "== here (NOSHAPE_LAND_LOCAL=1 or vibe3 unreachable)"
  tools/tiers/land-checks.sh "$pkgs" "$cf"; rc=$?
fi
rm -f "$cf"
[ $rc -eq 0 ] || exit 1
# Main may have moved while we tested: rebase again; re-test only if the new commits touch the crates we edit.
tested=$(git rev-parse origin/main)
git fetch -q origin
if ! git merge-base --is-ancestor origin/main HEAD; then
  moved=$(git diff --name-only "$tested" origin/main)
  git rebase -q origin/main || { git rebase --abort; fail "rebase conflict after testing"; }
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
  [ -z "$hit" ] || fail "main changed the crates you edit while testing ($hit): re-run land.sh"
fi
git push -q origin HEAD:main || fail "push rejected (main moved): re-run land.sh"
echo "LANDED $(git rev-parse --short HEAD)"
