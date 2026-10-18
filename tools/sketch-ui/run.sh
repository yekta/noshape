#!/bin/bash
# usage: run.sh [doc]  -- start noshape on :106 with a fresh empty doc
pkill -f "[t]arget-skb/debug/noshape"
sleep 0.5
DOC=${1:-/tmp/skb/x.noshape}
rm -f "$DOC" "$DOC"-*
cd /tmp/skb
NOSHAPE_NO_NATIVE_DIALOGS=1 DISPLAY=:106 timeout 600 /root/target-skb/debug/noshape $DOC > /tmp/skb/app.log 2>&1 &
for i in $(seq 1 60); do
  sleep 0.5
  if DISPLAY=:106 xdotool search --name "noshape|x.noshape|Part Studio" >/dev/null 2>&1; then break; fi
done
sleep 4
