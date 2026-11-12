#!/bin/bash
# gate.sh <branch|commit> [--jobs N] [--threads T] [--no-suite] [--no-speed] [--host H]: the Boolean's land gate in one
# command, one verdict line at the end (VERDICT PASS / CHECK / FAIL: why). Against main where the branch leaves it
# (its merge base with origin/main), on the build host:
#  - every Boolean corpus, parity56, shfall, the knurls and gears replayed (docs/handoff/bool.md's gate), each
#    distinct case once, in shards over the host's cores;
#  - the model suite, its documents in shards;
#  - our time on k25-15, k25-25 and cf-spur-gear, main's binary and the branch's alternating (best of 5; slower beyond
#    main's own round-to-round spread is CHECK);
#  - the branch's kernel, model and model-core tests (FAIL on any).
# Main's results are cached on the host by what they are built from (the crates' git trees, Cargo.lock, the OCCT
# libraries, the corpora, the models), so a gate usually replays the branch alone; the branch's are kept too (once it
# lands they are main's). FAIL: a case that differs from OCCT or fails where main's doesn't, a crash, a document
# matching fewer features. CHECK: a case main takes declined, a document's outcome changed, a timing case >5 % slower.
# The full output: /root/<caller>-scratch/gate-<sha>.log. One gate at a time per host (a lock); builds through rb.sh.
# gate.sh --main: main's results (replay, suite) worked out and cached ahead of the next gate, nothing done when they
# are (an hourly cron: main's kernel moves several times a day, and the first gate after each paid main's side).
# gate.sh --nightly-kall: kall's knurls (45-50 GB a process, not in the per-change gate) replayed on main alone, one
# case at a time, against the night before: bool2's cron at 04:30 UTC (lead 11-12). Its verdict line goes to
# /tmp/lead/kall-status and /root/<caller>-scratch/kall-nightly.log, a FAIL also to /root/bool-scratch/KALL-FAIL.
set -uo pipefail
# (Run from a copy: bash reads a script as it goes, and an edit to the checkout mid-gate broke one run.)
if [ -z "${GATE_HERE:-}" ]; then
  GATE_HERE=$(cd "$(dirname "$0")" && pwd); export GATE_HERE
  copy=$(mktemp /tmp/bool-gate-XXXXXX.sh); cp "$0" "$copy"; exec bash "$copy" "$@"
fi
rm -f "$0"
here=$GATE_HERE
repo=$(git -C "$here" rev-parse --show-toplevel)
caller=$(basename "$repo")
[ $# -ge 1 ] || { sed -n '2,13p' "$0"; exit 2; }
target=$1; shift
# (vibe4 left 11-12 19:55; the land queue and the gates moved to vibe5.)
jobs=8; threads=2; suite=1; spd=1; host=${NOSHAPE_GATE_HOST:-${NOSHAPE_RB_HOST:-vibe5}}; nightly=0; mainonly=0
[ "$target" = --nightly-kall ] && { nightly=1; target=origin/main; suite=0; spd=0; }
[ "$target" = --main ] && { mainonly=1; target=origin/main; spd=0; }
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

# Main alone, cached ahead of gates.
if [ $mainonly = 1 ]; then
  if cached "$bkey_r" replay && cached "$bkey_s" suite; then say "main ${base:0:9}: already cached"; exit 0; fi
  wt_base=/root/projects/noshape-wt/bool-gate-base
  [ -d "$wt_base" ] || git -C "$repo" worktree add -q --detach "$wt_base" origin/main >> "$log" 2>&1
  git -C "$wt_base" checkout -q -f --detach "$base" && git -C "$wt_base" clean -fdq
  ops=0; cached "$bkey_s" suite || ops=1
  cmd="cargo test --release -p noshape-kernel --test all --no-run"; [ $ops = 1 ] && cmd="$cmd && cargo test --release -p noshape-ops --test all --no-run"
  (cd "$wt_base" && NOSHAPE_RB_HOST=$host tools/tiers/rb.sh bash -c "$cmd" > "$scratch/gate-build-base.log" 2>&1) || { say "main doesn't build"; exit 1; }
  bins=$(grep -E 'Executable tests/all.rs' "$scratch/gate-build-base.log" | sed -n 's|.*(\(.*\)).*|\1|p')
  bk=$(echo "$bins" | sed -n 1p); bo=-; [ $ops = 1 ] && bo=$(echo "$bins" | sed -n 2p)
  ssh -o BatchMode=yes "$host" "mkdir -p ~/agents/bool-gate/bin ~/agents/bool-gate/runs" && scp -q "$here/gate-host.py" "$host:agents/bool-gate/bin/gate-host.py"
  (cd "$wt_base" && NOSHAPE_RB_HOST=$host tools/tiers/rb.sh bash -c "cd ~ && python3 agents/bool-gate/bin/gate-host.py run --out agents/bool-gate/runs/main-${base:0:9} --jobs $jobs --threads $threads --role base:$bkey_r:$bkey_s:$bkey_o:$bk:$bo") >> "$log" 2>&1
  say "main ${base:0:9}: replay $(cached "$bkey_r" replay && echo cached || echo 'not cached (a shard without results)'), suite $(cached "$bkey_s" suite && echo cached || echo 'not cached'), $(( ($(date +%s) - t0) / 60 )) min"
  exit 0
fi

# The nightly kall run: main's kernel tests built in main's worktree, kall replayed alone, compared with the night before.
if [ $nightly = 1 ]; then
  wt_base=/root/projects/noshape-wt/bool-gate-base
  [ -d "$wt_base" ] || git -C "$repo" worktree add -q --detach "$wt_base" origin/main >> "$log" 2>&1
  git -C "$wt_base" checkout -q -f --detach "$base" && git -C "$wt_base" clean -fdq
  build() { (cd "$1" && NOSHAPE_RB_HOST=$host tools/tiers/rb.sh bash -c "cargo test --release -p noshape-kernel --test all --no-run" > "$2" 2>&1); }
  build "$wt_base" "$scratch/gate-build-base.log" || { echo "VERDICT FAIL: main ${base:0:9} doesn't build" | tee -a "$scratch/kall-nightly.log"; exit 1; }
  bk=$(grep -E 'Executable tests/all.rs' "$scratch/gate-build-base.log" | sed -n 's|.*(\(.*\)).*|\1|p' | head -1)
  ssh -o BatchMode=yes "$host" "mkdir -p ~/agents/bool-gate/bin ~/agents/bool-gate/runs ~/agents/bool-gate/nightly" && scp -q "$here/gate-host.py" "$host:agents/bool-gate/bin/gate-host.py"
  night=$(date -u +%m%d); corp=${NOSHAPE_NIGHTLY_CORPORA:-kall}  # (another corpus: to try the run)
  out="agents/bool-gate/runs/kall-$night"
  (cd "$wt_base" && NOSHAPE_RB_HOST=$host tools/tiers/rb.sh bash -c "cd ~ && python3 agents/bool-gate/bin/gate-host.py run --out $out --jobs 1 --threads 8 --corpora $corp --role main:kall-$night-${base:0:9}:-:$bkey_o:$bk:- && cp $out/kall-$night-${base:0:9}-replay.tsv agents/bool-gate/nightly/kall-$night.tsv && prev=\$(ls agents/bool-gate/nightly/kall-*.tsv | grep -v kall-$night.tsv | tail -1) && if [ -n \"\$prev\" ]; then python3 agents/bool-gate/bin/gate-host.py cmpfiles \$prev agents/bool-gate/nightly/kall-$night.tsv; else echo \"VERDICT PASS: the first night, nothing to compare: \$(cut -f4 agents/bool-gate/nightly/kall-$night.tsv | sort | uniq -c | tr -s ' ' | paste -sd, -)\"; fi") >> "$log" 2>&1
  v=$(grep '^VERDICT' "$log" | tail -1); [ -n "$v" ] || v="VERDICT FAIL: the nightly kall run gave no verdict ($log)"
  line="$(date -u +%m-%d) kall on main ${base:0:9}: $v"
  echo "$line" | tee -a "$scratch/kall-nightly.log"; mkdir -p /tmp/lead; echo "$line" > /tmp/lead/kall-status
  case $v in "VERDICT PASS"*) exit 0;; "VERDICT CHECK"*) exit 3;; *) echo "$line" >> /root/bool-scratch/KALL-FAIL; exit 1;; esac
fi

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
# (The build's test binaries in its order: the kernel's first, the ops' second.)
exe() { grep -E 'Executable tests/all.rs' "$1" | sed -n 's|.*(\(.*\)).*|\1|p' | sed -n "$2p"; }
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
bk=$(exe "$scratch/gate-build-base.log" 1); nk=$(exe "$scratch/gate-build-new.log" 1)
bo=-; no=-
[ $need_bs = 1 ] && bo=$(exe "$scratch/gate-build-base.log" 2)
[ $need_ns = 1 ] && no=$(exe "$scratch/gate-build-new.log" 2)
[ -n "$bk" ] && [ -n "$nk" ] || { echo "VERDICT FAIL: no test binary in the build output ($scratch/gate-build-*.log)"; exit 1; }
say "built in $(( $(date +%s) - t0 )) s; running"

# The branch's own tests of the crates the Boolean reaches, as the land queue runs them (a model test catches what no
# corpus has: a round's torus split at its seam, 11-12): the kernel's, the model's, model-core's.
tests="cargo test --release -p noshape-kernel --test all && cargo test --release -p noshape-model --test all && cargo test --release -p noshape-model-core --lib"
# (Beside the run, not before it: 11 min of a 40-min warm gate.)
(cd "$wt_new" && NOSHAPE_RB_HOST=$host tools/tiers/rb.sh bash -c "$tests" > "$scratch/gate-tests.log" 2>&1) & ptests=$!

# The run: the gate's host script (this checkout's) from the branch's worktree.
ssh -o BatchMode=yes "$host" "mkdir -p ~/agents/bool-gate/bin ~/agents/bool-gate/runs" && scp -q "$here/gate-host.py" "$host:agents/bool-gate/bin/gate-host.py"
out="agents/bool-gate/runs/$caller-${new:0:9}-$(date -u +%m%d-%H%M%S)"
args=(--out "$out" --jobs "$jobs" --threads "$threads" --cmp base new
  --role "base:$bkey_r:$bkey_s:$bkey_o:$bk:$bo" --role "new:$nkey_r:$nkey_s:$nkey_o:$nk:$no")
# (A branch that leaves the kernel's crates alone times the same as main.)
[ $spd = 1 ] && [ "$base_rk" != "$new_rk" ] && args+=(--speed "$bk" "$nk")
(cd "$wt_new" && NOSHAPE_RB_HOST=$host tools/tiers/rb.sh bash -c "cd ~ && python3 agents/bool-gate/bin/gate-host.py run $(printf '%q ' "${args[@]}")") 2>&1 | tee -a "$log" | grep -E '^(gate-host|VERDICT|replay|model suite|speed)' >&2
wait $ptests; rt=$?
grep -E '^test result|FAILED$' "$scratch/gate-tests.log" >> "$log"
failed=$(grep -E '^test .* FAILED$' "$scratch/gate-tests.log" | sed 's/^test //; s/ \.\.\. FAILED$//' | head -3 | paste -sd, -)
say "crate tests: $([ $rt = 0 ] && echo pass || echo "fail ${failed:-(build or run; $scratch/gate-tests.log)}")"
ssh -o BatchMode=yes "$host" "find ~/agents/bool-gate/runs -mindepth 1 -maxdepth 1 -mtime +2 -exec rm -rf {} +" 2>/dev/null
v=$(grep '^VERDICT' "$log" | tail -1)
[ -n "$v" ] || v="VERDICT FAIL: the run gave no verdict (see $log)"
[ $rt = 0 ] || v="VERDICT FAIL: crate tests: ${failed:-see $scratch/gate-tests.log} (the run: ${v#VERDICT })"
echo "$v (branch ${new:0:9} vs main ${base:0:9}, $(( ($(date +%s) - t0) / 60 )) min; $log)"
case $v in "VERDICT PASS"*) exit 0;; "VERDICT CHECK"*) exit 3;; *) exit 1;; esac
