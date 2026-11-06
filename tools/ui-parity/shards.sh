#!/bin/bash
# The whole interaction suite (every scenario with a live recording) in parallel shards, one Xvfb display each, then
# one compare over all of it: the daily score (docs/PARITY-SUITES.md, docs/parity/SCORE.md).
#   tools/ui-parity/shards.sh <noshape binary> <out dir> [shards (default 6)] [first display (default 371)]
# e.g. on an idle vibe host, from a checkout at the green commit: tools/ui-parity/shards.sh /root/green/noshape
# /tmp/uip-$(date -u +%m%d). Prints the score; <out dir>/score.log has every scenario and the differences by key.
set -u
BIN=$1; OUT=$2; N=${3:-6}; D0=${4:-371}
cd "$(dirname "$0")/../.."
mkdir -p "$OUT"
mapfile -t names < <(for f in tests/ui-parity/live/*.json; do n=$(basename "$f" .json); [[ -f tests/ui-parity/$n.json ]] && echo "$n"; done | sort)
echo "${#names[@]} scenarios, $N shards"
pids=(); xpids=()
for k in $(seq 0 $((N - 1))); do
  d=$((D0 + k))
  Xvfb :$d -screen 0 1920x1080x24 -nolisten tcp > /dev/null 2>&1 &
  xpids+=($!)
  sleep 1
  shard=(); for i in "${!names[@]}"; do (( i % N == k )) && shard+=("${names[$i]}"); done
  python3 -u tools/ui-parity/run.py --app "$BIN" --display :$d --out "$OUT" "${shard[@]}" > "$OUT/shard-$k.log" 2>&1 &
  pids+=($!)
done
wait "${pids[@]}"
kill "${xpids[@]}" 2>/dev/null
python3 -u tools/ui-parity/run.py --compare-only --out "$OUT" "${names[@]}" > "$OUT/score.log" 2>&1
grep -v ": match" "$OUT/score.log"
