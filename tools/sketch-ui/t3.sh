#!/bin/bash
$(dirname "$0")/fresh.sh
export DISPLAY=:106
S=$(dirname "$0")/shot.sh
# two lines at an angle
xdotool mousemove 800 600 click 1; sleep 0.3; xdotool mousemove 1000 600 click 1; sleep 0.3; xdotool mousemove 1100 450 click 1; sleep 0.3; xdotool key Escape Escape; sleep 0.5
# dimension tool: pick both lines, preview
xdotool key d; sleep 0.3
xdotool mousemove 900 600 click 1; sleep 0.3; xdotool mousemove 1050 525 click 1; sleep 0.3; xdotool mousemove 1100 560; sleep 2; $S /tmp/skb/d1.png
xdotool click 1; sleep 2; $S /tmp/skb/d2.png
xdotool type 30; xdotool key Return; sleep 2; $S /tmp/skb/d3.png
xdotool key Escape; sleep 0.3; xdotool key Escape; sleep 0.3
# select first line and check toolbar; right click context menu
xdotool mousemove 880 600 click 1; sleep 1.5; $S /tmp/skb/d4.png
xdotool mousemove 880 600 click 3; sleep 1.5; $S /tmp/skb/d5.png
xdotool key Escape; sleep 0.5; xdotool key space; sleep 1.5; $S /tmp/skb/d6.png
