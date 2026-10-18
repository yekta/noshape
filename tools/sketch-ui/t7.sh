#!/bin/bash
$(dirname "$0")/fresh.sh
export DISPLAY=:106
S=$(dirname "$0")/shot.sh
c() { xdotool mousemove $1 $2; sleep 0.3; xdotool click 1; sleep 0.3; }
# polygon dropdown
c 431 58; sleep 0.8; $S /tmp/skb/k0.png
c 470 92; sleep 0.5
c 700 450; xdotool mousemove 780 450; sleep 0.3; xdotool type 5; sleep 0.3; c 780 450; sleep 1.5; $S /tmp/skb/k1.png
xdotool key Escape Escape; sleep 0.3
# line dropdown -> midpoint line (2nd item)
c 235 58; sleep 0.8; c 270 121; sleep 0.5
c 1000 450; c 1100 400; sleep 1.5; $S /tmp/skb/k2.png
xdotool key Escape Escape; sleep 0.3
# 3 point arc semicircle: a, start, end, then near apex
xdotool key a; sleep 0.3; c 700 700; c 850 700; xdotool mousemove 777 630; sleep 0.3; xdotool mousemove 776 626; sleep 1.5; $S /tmp/skb/k3.png; xdotool click 1; sleep 1; xdotool key Escape Escape
