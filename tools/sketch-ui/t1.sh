#!/bin/bash
$(dirname "$0")/fresh.sh
export DISPLAY=${DISPLAY:-:103}
xdotool mousemove 800 450 mousedown 1; sleep 0.2; xdotool mousemove 850 430; sleep 0.2; xdotool mousemove 900 400; sleep 0.3; xdotool mouseup 1; sleep 0.5
xdotool mousemove 1000 380; sleep 0.4
$(dirname "$0")/shot.sh /tmp/skb/a.png
xdotool mousemove 800 650 click 1; sleep 0.3; xdotool mousemove 950 650 click 1; sleep 0.3; xdotool mousemove 950 750 click 1; sleep 0.3; xdotool mousemove 1050 760; sleep 0.4
$(dirname "$0")/shot.sh /tmp/skb/b.png
xdotool key Escape; sleep 0.3; xdotool mousemove 1060 700; sleep 0.3; $(dirname "$0")/shot.sh /tmp/skb/c.png
cd /tmp/skb; for f in a b c; do convert $f.png -crop 800x500+500+250 ${f}c.png; done; convert ac.png bc.png cc.png +append ab.png
