#!/usr/bin/env bash
set -euo pipefail
declare scene
craft=$scene/craft
ctl scene new "$craft" --id org.example.craft >/dev/null
ctl scene add-layer "$craft" solid --id card >/dev/null
ctl scene add-layer "$craft" solid --id hole >/dev/null
ctl scene effects "$craft" card add vignette --param intensity=0.6 --name edge || fail "no effect started the stack"
ctl scene effects "$craft" card add film_grain --at 0 || fail "no second effect was added"
ctl scene effects "$craft" card set 1 --param radius=1.5 --enabled true || fail "the effect was not set"
if ctl scene effects "$craft" card set 1 --param intensity=4 2>/dev/null; then fail "an intensity of 4 was kept"; fi
ctl scene effects "$craft" card mask 1 --from hole --channel luminance --invert true || fail "the effect was not masked"
ctl scene effects "$craft" card move 1 0 || fail "the effect was not moved"
ctl scene effects "$craft" card remove 1 || fail "the effect was not removed"
ctl scene add "$craft" /scene/layers/1/effects/- '{"effect": "vignette"}' || fail "the list of effects was not created"
ctl scene preview "$craft" -o preview:1 --time 0.5 || fail "a scene with masked effects was not previewed"
ctl scene add-layer "$craft" particles --id dust >/dev/null
ctl scene particles "$craft" sparks create --parent dust --on static || fail "no child system was created"
ctl scene particles "$craft" dust settings --capacity 300 --seed 4 --prewarm 1 --space world --rate 30 || fail "the system was not set"
if ctl scene particles "$craft" dust settings --prewarm 900 2>/dev/null; then fail "a warm-up of 900 s was kept"; fi
ctl scene particles "$craft" dust start --life 1,3 --size 2,5 --color '#ffffff,#88ccff' --velocity -10,0:10,-40 ||
  fail "the starting ranges were not set"
ctl scene particles "$craft" dust start --clear velocity || fail "the velocity range was not cleared"
ctl scene particles "$craft" dust force gravity 0,40 || fail "no gravity was added"
ctl scene particles "$craft" dust force drag 0.2 --index 0 || fail "the force was not replaced"
ctl scene particles "$craft" dust force oscillate size --freq 1,2 --scale 0.5,1 || fail "no swing was added"
ctl scene particles "$craft" dust force attractor 40,30 --strength 30 --falloff linear || fail "no attractor was added"
ctl scene particles "$craft" dust force turbulence --strength 10 --scale 0.01 || fail "no turbulence was added"
ctl scene particles "$craft" dust force remove 1 || fail "the force was not removed"
ctl scene particles "$craft" dust child add sparks --on death --probability 0.5 || fail "no child was added"
ctl scene particles "$craft" dust child set 1 sparks --on spawn --max 3 || fail "the child was not replaced"
ctl scene particles "$craft" dust child remove 0 || fail "the child was not removed"
ctl scene particles "$craft" dust size-over-life 0:1 0.5:2 1:0 || fail "the size over life was not set"
ctl scene particles "$craft" dust color-over-life '0:#ffffff' '1:#ff000000' || fail "the color over life was not set"
ctl scene particles "$craft" dust render --stretch 0.1,2,40 --frames random --fps 8 --mix true --brightness 2.5 ||
  fail "the look of the particles was not set"
if ctl scene particles "$craft" dust render --brightness 99 2>/dev/null; then fail "a brightness of 99 was kept"; fi
if ctl scene particles "$craft" sparks delete 2>/dev/null; then fail "a system in use was deleted"; fi
ctl scene particles "$craft" lone create || fail "no lone system was created"
ctl scene particles "$craft" lone delete || fail "a lone system was not deleted"
ctl scene preview "$craft" -o preview:1 --time 0.5 || fail "a scene with every particle field was not previewed"
