#!/bin/bash
$(dirname "$0")/fresh.sh
export DISPLAY=:106
S=$(dirname "$0")/shot.sh
c() { xdotool mousemove $1 $2; sleep 0.4; xdotool click 1; sleep 0.4; }
xdotool key g; c 600 400; c 800 550; xdotool key Escape Escape; sleep 0.5
# box select window left->right around whole rect
xdotool mousemove 560 360 mousedown 1; sleep 0.2; xdotool mousemove 700 480; sleep 0.2; xdotool mousemove 850 600; sleep 0.3; xdotool mouseup 1; sleep 1.5; $S /tmp/skb/m1.png
xdotool key ctrl+c; sleep 0.3; xdotool key space; sleep 1.2; $S /tmp/skb/m2.png
xdotool key ctrl+v; sleep 2; $S /tmp/skb/m3.png
# hover top edge to show glyphs, right click glyph
xdotool key space; sleep 0.3; xdotool mousemove 700 400; sleep 2; $S /tmp/skb/m4.png
