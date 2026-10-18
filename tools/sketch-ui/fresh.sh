#!/bin/bash
$(dirname "$0")/run.sh
$(dirname "$0")/sketch_top.sh
export DISPLAY=:106
xdotool mousemove 900 500; xdotool key n; sleep 1.5
