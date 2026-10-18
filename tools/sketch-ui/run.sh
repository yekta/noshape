#!/bin/bash
# usage: run.sh [doc]  -- start noshape on ${DISPLAY:-:103} with a fresh empty doc
pkill -f "[t]arget-skb/debug/noshape"
sleep 0.5
DOC=${1:-/tmp/skb/x.noshape}
rm -f "$DOC" "$DOC"-*
mkdir -p /tmp/skb
cd /tmp/skb
setsid env NOSHAPE_NO_NATIVE_DIALOGS=1 LP_NUM_THREADS=${LP_NUM_THREADS:-2} DISPLAY=${DISPLAY:-:103} /root/target-skb/debug/noshape "$DOC" < /dev/null > /tmp/skb/app.log 2>&1 &
printf '%s\n' "$!" > /tmp/skb/app.pid
for i in $(seq 1 60); do
  sleep 0.5
  if DISPLAY=${DISPLAY:-:103} xdotool search --name "noshape|x.noshape|Part Studio" >/dev/null 2>&1; then break; fi
done
sleep 4
