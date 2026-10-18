#!/bin/bash
$(dirname "$0")/fresh.sh
export DISPLAY=${DISPLAY:-:103}
S=$(dirname "$0")/shot.sh
c() { xdotool mousemove $1 $2; sleep 0.3; xdotool click 1; sleep 0.3; }
xdotool key g; c 600 400; c 800 550; xdotool key Escape Escape; sleep 0.5
xdotool key m; sleep 0.3; xdotool mousemove 650 350; sleep 0.5; xdotool mousedown 1; sleep 0.3; xdotool mousemove 650 420; sleep 1.5; $S /tmp/skb/h1.png
for y in 450 500 520; do xdotool mousemove 650 $y; sleep 0.3; done; sleep 1.2; $S /tmp/skb/h2.png
xdotool mousemove 650 580; sleep 0.3; xdotool mouseup 1; sleep 1.5; $S /tmp/skb/h3.png
