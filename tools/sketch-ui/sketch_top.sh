#!/bin/bash
# maximize window and open a sketch on Top
export DISPLAY=:106
W=$(xdotool search --name "noshape" | head -1)
xdotool windowmove $W 0 0 windowsize $W 1600 1000
sleep 1
# click "Top" in feature list then Shift+S
xdotool mousemove 96 207 click 1
sleep 0.5
xdotool key shift+s
sleep 2
