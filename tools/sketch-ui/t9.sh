#!/bin/bash
$(dirname "$0")/fresh.sh
export DISPLAY=:106
S=$(dirname "$0")/shot.sh
c() { xdotool mousemove $1 $2; sleep 0.4; xdotool click 1; sleep 0.4; }
xdotool key g; c 600 400; c 800 550; sleep 0.5; xdotool type "=10*3"; xdotool key Return; sleep 0.8; xdotool key Escape; sleep 0.3; xdotool key Escape Escape; sleep 1
$S /tmp/skb/n0.png
# right-click the dimension label (approx 698,577)
xdotool mousemove 698 577; sleep 0.8; xdotool click 3; sleep 1.5; $S /tmp/skb/n1.png
xdotool key Escape; sleep 0.3; xdotool mousemove 1000 800 click 1; sleep 0.5
# hover left edge, glyph appears; right click glyph near (606+? )
xdotool mousemove 600 470; sleep 2; $S /tmp/skb/n2.png
