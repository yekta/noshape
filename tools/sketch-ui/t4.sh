#!/bin/bash
$(dirname "$0")/fresh.sh
export DISPLAY=:106
S=$(dirname "$0")/shot.sh
m() { xdotool mousemove $1 $2; sleep 0.25; }
c() { xdotool mousemove $1 $2; sleep 0.25; xdotool click 1; sleep 0.25; }
# rectangle then fillet a corner then chamfer another
xdotool key g; c 700 450; c 900 600; xdotool key Escape; sleep 0.3
xdotool key shift+f; sleep 0.3; c 700 450; sleep 1; xdotool type 5; xdotool key Return; sleep 0.8; c 900 450; sleep 0.5; xdotool key Escape Escape; sleep 0.5
# polygon: choose from toolbar dropdown? use activate via S? -> skip, use slot on a line
xdotool key l; c 1000 700; c 1200 650; xdotool key Escape Escape; sleep 0.3
# select the line then use slot via toolbar: Offset group dropdown at ~ (797,58)
m 1100 675; xdotool click 1; sleep 0.5
xdotool mousemove 797 58 click 1; sleep 1; $S /tmp/skb/f0.png
