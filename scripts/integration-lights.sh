#!/usr/bin/env bash
set -euo pipefail
declare draft
ctl scene add-layer "$draft" image --id wall --source leaf >/dev/null || fail "no image layer was added"
ctl scene light "$draft" lamp add --type spot --at 40,30 || fail "no spot light was added"
ctl scene light "$draft" lamp set --intensity 3 --radius 200 --cone 20,40 --aim 80,60 --color '#ffcc88' --tilt 30 ||
  fail "the spot light was not set"
if ctl scene light "$draft" lamp set --length 50 2>/dev/null; then fail "a spot light took a length"; fi
if ctl scene light "$draft" wall set --intensity 2 2>/dev/null; then fail "an image was set as a light"; fi
ctl scene light "$draft" tube add --type tube || fail "no tube light was added"
ctl scene light "$draft" tube set --length 120 --at 60,40 --z 30 || fail "the tube light was not set"
ctl scene light "$draft" lamp falloff | grep -q '"curve"' || fail "the falloff of the light was not plotted"
ctl scene lights "$draft" | grep -q '"outer"' || fail "the cone of the spot light was not listed"
ctl scene lights "$draft" | grep -q '"ends"' || fail "the ends of the tube light were not listed"
ctl scene lights "$draft" --cost --quality q0 | grep -q '"baked": true' || fail "the lowest level does not bake the lighting"
ctl scene light "$draft" tube mute || fail "the tube light was not muted"
ctl scene light "$draft" lamp solo || fail "the spot light was not lit alone"
ctl scene lights "$draft" | grep -q '"solo": "lamp"' || fail "the solo light was not listed"
ctl scene light "$draft" lamp solo --off || fail "the solo light was not let go"
ctl scene light "$draft" tube mute --off || fail "the tube light was not brought back"
ctl scene normals "$draft" leaf --generate --strength 3 --blur 1 --layer wall --id leaf_normals ||
  fail "no normal map was made"
ctl scene normals "$draft" leaf --preview | grep -q '"relief"' || fail "the relief of the normal map was not shown"
ctl scene show "$draft" /scene/layers | grep -q '"leaf_normals"' || fail "the layer does not take the normal map"
ctl scene lighting "$draft" wall --roughness 0.3 --reflection 0.2 --emissive 0.5 --strength 0.9 ||
  fail "the lighting of the layer was not set"
if ctl scene lighting "$draft" wall --roughness 3 2>/dev/null; then fail "a roughness of 3 was kept"; fi
ctl scene paint-map "$draft" wall roughness --points '0.2,0.5;0.8,0.5' --radius 0.1 --value 0.2 ||
  fail "no roughness was painted"
ctl scene paint-map "$draft" wall emissive --points '0.5,0.5' --radius 0.2 || fail "no glow was painted"
ctl scene paint-map "$draft" wall emissive --heat | grep -q '"heat"' || fail "the painted glow was not shown"
ctl scene ambient "$draft" --ambient '#202020' --sky '#6080ff' || fail "the ambient light was not set"
ctl scene ambient "$draft" --preview | grep -q '"sphere"' || fail "the hemisphere was not shown"
ctl scene validate "$draft" >/dev/null || fail "the lit scene is not valid"
ctl scene show "$draft" /manifest/requires/features | grep -q lighting2d || fail "lighting left lighting2d out"
ctl scene preview "$draft" -o preview:1 --time 0.5 || fail "a lit scene was not previewed"
ctl scene light "$draft" tube remove || fail "the tube light was not removed"
ctl scene light "$draft" lamp remove || fail "the spot light was not removed"
ctl scene ambient "$draft" --clear || fail "the ambient light was not cleared"
ctl scene remove-layer "$draft" wall || fail "the lit layer was not removed"
ctl scene validate "$draft" >/dev/null || fail "removing the lights left the scene invalid"
