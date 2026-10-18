#!/bin/bash
$(dirname "$0")/fresh.sh
export DISPLAY=:106
S=$(dirname "$0")/shot.sh
m() { xdotool mousemove $1 $2; sleep 0.25; }
c() { xdotool mousemove $1 $2; sleep 0.3; xdotool click 1; sleep 0.3; }
c 1000 700; c 1200 650; xdotool key Escape Escape; sleep 0.3
c 1100 675; sleep 0.3
xdotool mousemove 797 58 click 1; sleep 1; xdotool mousemove 820 121; sleep 0.3; xdotool click 1; sleep 2; $S /tmp/skb/g1.png
xdotool type 8; xdotool key Return; sleep 2; $S /tmp/skb/g2.png
xdotool key Escape Escape; sleep 0.3
# rectangle + region click
xdotool key g; c 600 400; c 800 550; xdotool key Escape Escape; sleep 0.5
c 700 470; sleep 1.5; $S /tmp/skb/g3.png
# trim drag across rectangle top/bottom
xdotool key m; sleep 0.3; xdotool mousemove 650 350 mousedown 1; sleep 0.2; for y in 400 450 500 550 600; do xdotool mousemove 650 $y; sleep 0.15; done; xdotool mouseup 1; sleep 2; $S /tmp/skb/g4.png
