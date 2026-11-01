#!/bin/bash
# rb.sh: run a build or test command on a build server (vibe4 or vibe5 per tools/tiers/rb-hosts) instead of this machine,
# which agents' builds kept at 2-3x its cores (11-01: building agents spent ~80% of their day waiting on compiles).
# Syncs this worktree as it is (uncommitted changes included; not .git, not target dirs) to vibe4:~/agents/<worktree>/src,
# runs the command there with that worktree's own target dir (~/agents/<worktree>/target, kept warm between runs) and
# streams its output; the exit code is the command's.
#   tools/tiers/rb.sh cargo test -p noshape-brep --lib
#   tools/tiers/rb.sh cargo test -p noshape-model --test all knurl -- --ignored
#   tools/tiers/rb.sh --get noshape cargo build -p noshape   # then copies target/debug/noshape back into
#                                                            # $CARGO_TARGET_DIR/debug/ to run the app here
# NOSHAPE_RB_HOST overrides the host; NOSHAPE_RB_LOCAL=1 (or the host unreachable) runs the command here instead.
set -uo pipefail
cd "$(git rev-parse --show-toplevel)"
name=$(basename "$PWD"); dir="agents/$name"
# The host: NOSHAPE_RB_HOST, else this worktree's line in tools/tiers/rb-hosts, else vibe4 (also when that one is down).
host=${NOSHAPE_RB_HOST:-}
if [ -z "$host" ]; then
  while read -r pat h; do
    case "$pat" in ''|'#'*) continue;; esac
    # shellcheck disable=SC2254
    case "$name" in $pat) host=$h; break;; esac
  done < tools/tiers/rb-hosts 2>/dev/null
fi
host=${host:-vibe4}
[ "$host" != vibe4 ] && ! ssh -o BatchMode=yes -o ConnectTimeout=5 "$host" true 2>/dev/null && { echo "== rb: $host unreachable, using vibe4" >&2; host=vibe4; }
get=()
while [ "${1:-}" = --get ]; do get+=("$2"); shift 2; done
[ $# -gt 0 ] || { echo "usage: rb.sh [--get <bin>]... <command...>"; exit 2; }
if [ "${NOSHAPE_RB_LOCAL:-0}" = 1 ] || ! ssh -o BatchMode=yes -o ConnectTimeout=5 "$host" true 2>/dev/null; then
  echo "== rb: here ($host unreachable or NOSHAPE_RB_LOCAL=1)" >&2; exec "$@"
fi
# Times (-t) kept so cargo rebuilds only what changed; deleted files go too (--delete).
ssh -o BatchMode=yes "$host" "mkdir -p ~/$dir/src" && rsync -a --delete --exclude=.git --exclude=/target --exclude='/target-*' -e "ssh -o BatchMode=yes" ./ "$host:$dir/src/" \
  || { echo "== rb: sync to $host failed" >&2; exit 1; }
cmd=$(printf '%q ' "$@")
# The remote run stops when this ssh does (-tt gives it a terminal, so a killed rb.sh kills the build there too).
ssh -q -tt -o BatchMode=yes "$host" ". ~/noshape-env.sh; export CARGO_TARGET_DIR=~/$dir/target; \
  [ -n \"\${NOSHAPE_CONFIG_DIR:-}\" ] || export NOSHAPE_CONFIG_DIR=\$(mktemp -d -t noshape-config-XXXXXX); \
  cd ~/$dir/src && nice -n 5 $cmd" 2>&1 | tr -d '\r'
rc=${PIPESTATUS[0]}
# A binary built there has that build's source paths compiled in (the app's icons and assets): make the same path
# here point at this worktree, so the copied app finds them (skasm 11-01).
if [ ${#get[@]} -gt 0 ]; then
  rhome=$(ssh -o BatchMode=yes "$host" 'echo $HOME'); mkdir -p "$rhome/$dir"; ln -sfn "$PWD" "$rhome/$dir/src"
fi
out="${CARGO_TARGET_DIR:-$PWD/target}/debug"
for b in "${get[@]}"; do
  mkdir -p "$out"
  rsync -a -e "ssh -o BatchMode=yes" "$host:$dir/target/debug/$b" "$out/$b" && echo "== rb: copied $b to $out/$b" >&2
done
exit $rc
