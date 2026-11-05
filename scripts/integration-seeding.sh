#!/usr/bin/env bash
set -euo pipefail
declare scene
craft=$scene/craft
ctl scene particles "$craft" dust emitter 0 --burst 20 --every 2 --periodic 1,2:0.5,1 --delay 0.5 --follow true --audio-gate true --inherit 0.3 ||
  fail "the emitter was not set"
ctl scene particles "$craft" dust emitter 0 --clear burst --audio-gate false || fail "the emitter values were not removed"
[ "$(ctl scene show "$craft" /scene/particles/dust/emitters/0/follow)" = '"cursor"' ] || fail "the emitter does not follow the pointer"
if ctl scene particles "$craft" dust emitter 0 --delay -1 2>/dev/null; then fail "a negative delay was kept"; fi
ctl scene particles "$craft" dust start --hsv 180,240 --saturation 0.5,1 --hue-steps 6 || fail "the particles took no hues"
ctl scene particles "$craft" dust start --palette '#ff0000,#00ff00' --palette-noise 0.1,0,0 || fail "the particles took no palette"
ctl scene particles "$craft" dust start --gradient '0:#000000' --gradient '1:#ffffff' || fail "the particles took no gradient"
if ctl scene show "$craft" /scene/particles/dust/init/palette >/dev/null 2>&1; then fail "the palette stayed beside the gradient"; fi
ctl scene particles "$craft" dust start --clear gradient || fail "the gradient was not removed"
ctl scene particles "$craft" dust window 0 0,0.2,0.8,1 || fail "the force took no window of the life"
if ctl scene particles "$craft" dust window 0 0.9,0.2,0.8,1 2>/dev/null; then fail "a window out of order was kept"; fi
ctl scene particles "$craft" dust window 0 none || fail "the window was not removed"
ctl scene particles "$craft" dust settings --lock size,speed || fail "nothing was locked"
if ctl scene particles "$craft" dust settings --lock alpha 2>/dev/null; then fail "a value the layer changes was locked"; fi
ctl scene particles "$craft" dust render --frames life --cycles 2 --order oldest || fail "the cycles and order were not set"
if ctl scene particles "$craft" dust render --order sideways 2>/dev/null; then fail "an unknown order was kept"; fi
ctl scene particles "$craft" dust force turbulence --strength 5 --scale 0.01 --time-scale 2 --mask 0.2,0.8 || fail "the turbulence took no mask"
if ctl scene particles "$craft" dust force turbulence --strength 5 --scale 0.01 --mask 0.2,3 2>/dev/null; then fail "a mask of 3 was kept"; fi
ctl scene layer "$craft" dust instance --point 10,20 --point 30,-40 || fail "the instance took no control points"
ctl scene particles "$craft" sparks rename embers || fail "the system was not renamed"
[ "$(ctl scene show "$craft" /scene/particles/dust/children/0/system)" = '"embers"' ] || fail "the child did not follow the new name"
ctl scene preview "$craft" -o preview:1 --time 0.5 || fail "a scene with every emitter and color field was not previewed"
