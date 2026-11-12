#!/bin/bash
# gate.sh <branch|commit> [--jobs N] [--threads T] [--no-suite] [--no-speed] [--host H]: the Boolean's land gate in one
# command, one verdict line at the end (VERDICT PASS / CHECK / FAIL: why). Against main where the branch leaves it
# (its merge base with origin/main), on the build host:
#  - every Boolean corpus, parity56, shfall, the knurls and gears replayed (docs/handoff/bool.md's gate), each
#    distinct case once, in shards over the host's cores;
#  - the model suite, its documents in shards;
#  - our time on k25-15, k25-25 and cf-spur-gear, main's binary and the branch's alternating (best of 3).
# Main's results are cached on the host by what they are built from (the crates' git trees, Cargo.lock, the OCCT
# libraries, the corpora, the models), so a gate usually replays the branch alone; the branch's are kept too (once it
# lands they are main's). FAIL: a case that differs from OCCT or fails where main's doesn't, a crash, a document
# matching fewer features. CHECK: a case main takes declined, a document's outcome changed, a timing case >5 % slower.
# The full output: /root/<caller>-scratch/gate-<sha>.log. One gate at a time per host (a lock); builds through rb.sh.
set -uo pipefail
here=$(cd "$(dirname "$0")" && pwd)
repo=$(git -C "$here" rev-parse --show-toplevel)
caller=$(basename "$repo")
[ $# -ge 1 ] || { sed -n '2,13p' "$0"; exit 2; }
target=$1; shift
jobs=8; threads=2; suite=1; spd=1; host=vibe4
while [ $# -gt 0 ]; do
  case $1 in
    --jobs) jobs=$2; shift;; --threads) threads=$2; shift;; --host) host=$2; shift;;
    --no-suite) suite=0;; --no-speed) spd=0;;
    *) echo "gate: unknown option $1"; exit 2;;
  esac
  shift
done
git -C "$repo" fetch -q origin || { echo "VERDICT FAIL: git fetch failed"; exit 1; }
new=$(git -C "$repo" rev-parse --verify -q "$target^{commit}") || { echo "VERDICT FAIL: no commit $target"; exit 1; }
base=$(git -C "$repo" merge-base "$new" origin/main)
scratch=/root/$caller-scratch; mkdir -p "$scratch"
log=$scratch/gate-${new:0:9}.log
t0=$(date +%s)
say() { echo "== gate $(date -u +%H:%M:%S): $*" | tee -a "$log" >&2; }
: > "$log"
say "branch ${new:0:9} ($target) against main ${base:0:9} on $host, $jobs jobs x $threads threads"

# What each test binary is built from: the crates it compiles (path dependencies, followed), the lock file, the
# toolchain, the OCCT patches. A commit that changes none of these gives the same results.
keys() {
  python3 - "$repo" "$1" <<'EOF'
import hashlib, re, subprocess, sys
repo, commit = sys.argv[1], sys.argv[2]
def show(p):
    r = subprocess.run(["git", "-C", repo, "show", f"{commit}:{p}"], capture_output=True, text=True)
    return r.stdout if r.returncode == 0 else ""
def tree(p):
    r = subprocess.run(["git", "-C", repo, "rev-parse", f"{commit}:{p}"], capture_output=True, text=True)
    return r.stdout.strip() if r.returncode == 0 else "none"
paths = dict(re.findall(r'^(noshape-[\w-]+)\s*=\s*\{\s*path\s*=\s*"([^"]+)"', show("Cargo.toml"), re.M))
def closure(crate):
    seen, todo = set(), [crate]
    while todo:
        c = todo.pop()
        if c in seen or c not in paths:
            continue
        seen.add(c)
        todo += re.findall(r'^(noshape-[\w-]+)', show(paths[c] + "/Cargo.toml"), re.M)
    return sorted(paths[c] for c in seen)
common = ["Cargo.lock", "Cargo.toml", "rust-toolchain.toml", ".cargo", "patches/occt", "crates/workspace-hack"]
# (OCCT's side of a replay: the kernel and the parts of crates/brep it calls with noshape's Boolean off (the mesher
# op_boolean's grouping meshes with), not the Boolean.)
occt = ["crates/kernel/src", "crates/kernel/cpp", "crates/kernel/build.rs", "crates/kernel/Cargo.toml", "crates/brep/Cargo.toml", "crates/brep/src/lib.rs", "crates/brep/src/nurbs.rs", "crates/brep/src/mesh"]
for paths in [closure("noshape-kernel") + common, closure("noshape-ops") + common + ["tests/parity-models"], occt + common]:
    parts = [f"{p} {tree(p)}" for p in paths]
    print(hashlib.sha1("\n".join(parts).encode()).hexdigest()[:16], end=" ")
print()
EOF
}
read -r base_rk base_sk base_ok < <(keys "$base")
read -r new_rk new_sk new_ok < <(keys "$new")
hostfp=$(ssh -o BatchMode=yes "$host" "python3 - fp" < "$here/gate-host.py") || { say "no fingerprint from $host"; echo "VERDICT FAIL: $host unreachable"; exit 1; }
rfp=$(echo "$hostfp" | python3 -c 'import json,sys; print(json.load(sys.stdin)["replay"][:12])')
sfp=$(echo "$hostfp" | python3 -c 'import json,sys; print(json.load(sys.stdin)["suite"][:12])')
key() { echo "$1-$2"; }
bkey_r=$(key "$base_rk" "$rfp"); nkey_r=$(key "$new_rk" "$rfp")
bkey_s=$(key "$base_sk" "$sfp"); nkey_s=$(key "$new_sk" "$sfp")
bkey_o=$(key "$base_ok" "$rfp"); nkey_o=$(key "$new_ok" "$rfp")
cached() { ssh -o BatchMode=yes "$host" "test -s ~/agents/bool-gate/cache/$1-$2.tsv"; }

# One gate per host at a time: its worktrees, target dirs and cores.
lock=/root/projects/noshape-wt/.bool-gate-$host.lock
exec 9> "$lock"
flock -n 9 || { say "waiting for another gate on $host"; flock 9; }

# The two worktrees (warm target dirs on the host): main's shared, the branch's per caller.
wt_base=/root/projects/noshape-wt/bool-gate-base
wt_new=/root/projects/noshape-wt/$caller-gate
for wt in "$wt_base" "$wt_new"; do
  [ -d "$wt" ] || git -C "$repo" worktree add -q --detach "$wt" origin/main >> "$log" 2>&1
done
git -C "$wt_base" checkout -q -f --detach "$base" && git -C "$wt_base" clean -fdq
git -C "$wt_new" checkout -q -f --detach "$new" && git -C "$wt_new" clean -fdq

# Builds: the kernel's tests always (the timing rounds run main's and the branch's), the ops tests when a suite
# result is missing. Main's and the branch's at once.
build() { # <worktree> <with ops 0/1> <out file>
  local cmd="cargo test --release -p noshape-kernel --test all --no-run"
  [ "$2" = 1 ] && cmd="$cmd && cargo test --release -p noshape-ops --test all --no-run"
  (cd "$1" && NOSHAPE_RB_HOST=$host tools/tiers/rb.sh bash -c "$cmd" > "$3" 2>&1)
}
exe() { grep -E 'Executable tests/all.rs' "$1" | sed -n "s|.*(\(.*/$2\)).*|\1|p" | tail -1; }
need_bs=0; need_ns=0
if [ $suite = 1 ]; then
  cached "$bkey_s" suite || need_bs=1
  cached "$nkey_s" suite || need_ns=1
fi
say "cache: main replay $(cached "$bkey_r" replay && echo hit || echo miss), suite $([ $suite = 0 ] && echo off || { [ $need_bs = 0 ] && echo hit || echo miss; }); branch replay $(cached "$nkey_r" replay && echo hit || echo miss), suite $([ $suite = 0 ] && echo off || { [ $need_ns = 0 ] && echo hit || echo miss; })"
build "$wt_base" $need_bs "$scratch/gate-build-base.log" & pb=$!
build "$wt_new" $need_ns "$scratch/gate-build-new.log" & pn=$!
wait $pb; rb=$?; wait $pn; rn=$?
cat "$scratch/gate-build-base.log" "$scratch/gate-build-new.log" >> "$log"
[ $rn = 0 ] || { say "the branch doesn't build (tail of $scratch/gate-build-new.log):"; tail -20 "$scratch/gate-build-new.log" >&2; echo "VERDICT FAIL: the branch doesn't build"; exit 1; }
[ $rb = 0 ] || { say "main doesn't build: $scratch/gate-build-base.log"; echo "VERDICT FAIL: main ${base:0:9} doesn't build here"; exit 1; }
bk=$(exe "$scratch/gate-build-base.log" 'all-[0-9a-f]*'); nk=$(exe "$scratch/gate-build-new.log" 'all-[0-9a-f]*')
# (cargo prints the kernel's binary first, the ops' second.)
bo=-; no=-
[ $need_bs = 1 ] && bo=$(grep -E 'Executable tests/all.rs' "$scratch/gate-build-base.log" | sed -n 's|.*(\(.*\)).*|\1|p' | grep -v "$bk" | tail -1)
[ $need_ns = 1 ] && no=$(grep -E 'Executable tests/all.rs' "$scratch/gate-build-new.log" | sed -n 's|.*(\(.*\)).*|\1|p' | grep -v "$nk" | tail -1)
[ -n "$bk" ] && [ -n "$nk" ] || { echo "VERDICT FAIL: no test binary in the build output ($scratch/gate-build-*.log)"; exit 1; }
say "built in $(( $(date +%s) - t0 )) s; running"

# The run: the gate's host script (this checkout's) from the branch's worktree.
ssh -o BatchMode=yes "$host" "mkdir -p ~/agents/bool-gate/bin ~/agents/bool-gate/runs" && scp -q "$here/gate-host.py" "$host:agents/bool-gate/bin/gate-host.py"
out="agents/bool-gate/runs/$caller-${new:0:9}-$(date -u +%m%d-%H%M%S)"
args=(--out "$out" --jobs "$jobs" --threads "$threads" --cmp base new
  --role "base:$bkey_r:$bkey_s:$bkey_o:$bk:$bo" --role "new:$nkey_r:$nkey_s:$nkey_o:$nk:$no")
# (A branch that leaves the kernel's crates alone times the same as main.)
[ $spd = 1 ] && [ "$base_rk" != "$new_rk" ] && args+=(--speed "$bk" "$nk")
(cd "$wt_new" && NOSHAPE_RB_HOST=$host tools/tiers/rb.sh bash -c "cd ~ && python3 agents/bool-gate/bin/gate-host.py run $(printf '%q ' "${args[@]}")") 2>&1 | tee -a "$log" | grep -E '^(gate-host|VERDICT|replay|model suite|speed)' >&2
ssh -o BatchMode=yes "$host" "find ~/agents/bool-gate/runs -mindepth 1 -maxdepth 1 -mtime +2 -exec rm -rf {} +" 2>/dev/null
v=$(grep '^VERDICT' "$log" | tail -1)
[ -n "$v" ] || v="VERDICT FAIL: the run gave no verdict (see $log)"
echo "$v (branch ${new:0:9} vs main ${base:0:9}, $(( ($(date +%s) - t0) / 60 )) min; $log)"
case $v in "VERDICT PASS"*) exit 0;; "VERDICT CHECK"*) exit 3;; *) exit 1;; esac
