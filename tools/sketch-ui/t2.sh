#!/bin/bash
$(dirname "$0")/fresh.sh
export DISPLAY=${DISPLAY:-:103}
S=$(dirname "$0")/shot.sh
# corner rectangle via g, type 30 Enter 20 Enter
xdotool key g; sleep 0.3
xdotool mousemove 700 600 click 1; sleep 0.3; xdotool mousemove 850 700 click 1; sleep 0.8
$S /tmp/skb/r1.png
xdotool type 30; xdotool key Return; sleep 0.8; $S /tmp/skb/r2.png
xdotool type 20; xdotool key Return; sleep 1; $S /tmp/skb/r3.png
# circle: c, click center, drag radius
xdotool key c; sleep 0.3
xdotool mousemove 1100 450 mousedown 1; sleep 0.2; xdotool mousemove 1140 450; sleep 0.2; xdotool mousemove 1160 470; sleep 0.2; xdotool mouseup 1; sleep 1
$S /tmp/skb/r4.png
# line chain then shift+a tangent arc
xdotool key l; sleep 0.3
xdotool mousemove 700 350 click 1; sleep 0.3; xdotool mousemove 850 350 click 1; sleep 0.3; xdotool key shift+a; sleep 0.2; xdotool mousemove 900 300; sleep 0.3; xdotool mousemove 920 250; sleep 1; $S /tmp/skb/r5.png
xdotool click 1; sleep 0.3; xdotool mousemove 800 200; sleep 1; $S /tmp/skb/r6.png
xdotool key Escape Escape; sleep 0.5
cd /tmp/skb; for f in r1 r2 r3 r4 r5 r6; do convert $f.png -crop 700x500+550+150 ${f}c.png; done; convert r1c.png r2c.png r3c.png +append top.png; convert r4c.png r5c.png r6c.png +append bot.png; convert top.png bot.png -append r.png
