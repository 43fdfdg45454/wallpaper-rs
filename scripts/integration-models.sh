#!/usr/bin/env bash
set -euo pipefail
declare draft ROOT
toy=$ROOT/tests/fixtures/toy.glb
ctl scene model-info "$toy" | grep -q '"triangles": 12' || fail "the model file was not described"
ctl scene model-info "$toy" | grep -q '"tier": "t0"' || fail "the model file gave no verdict per tier"
if ctl scene model-info "$ROOT/tests/fixtures/green-64x32.png" 2>/dev/null; then fail "a picture was read as a model"; fi
ctl scene import "$draft" "$toy" --id toy >/dev/null || fail "no model was imported"
ctl scene model "$draft" add doll --asset toy --at 0,0,-1 || fail "no model layer was added"
ctl scene model "$draft" set doll --rotation 0,30,0 --scale 1.5,1.5,1.5 || fail "the model was not placed"
if ctl scene model "$draft" set doll --scale 0,1,1 2>/dev/null; then fail "a model was scaled to nothing"; fi
if ctl scene model "$draft" add other --asset nothing 2>/dev/null; then fail "a missing asset was added as a model"; fi
ctl scene model "$draft" info doll | grep -q '"materials"' || fail "the model of the layer was not described"
ctl scene group3d "$draft" add world --layer doll || fail "no 3D group was made around the model"
ctl scene show "$draft" /scene/layers | grep -q '"space": "3d"' || fail "the group is not 3D"
ctl scene add-layer "$draft" solid --id card --into world >/dev/null || fail "no flat layer was added to the 3D group"
ctl scene group3d "$draft" depth-test card || fail "the flat layer does not test depth"
if ctl scene group3d "$draft" depth-test doll 2>/dev/null; then fail "a model was made to test depth as a flat layer"; fi
ctl scene group3d "$draft" show | grep -q '"arms"' || fail "the handles of the model were not shown"
ctl scene group3d "$draft" show | grep -q '"depth_test": true' || fail "the flat layer that tests depth was not listed"
ctl scene material "$draft" doll paint --shader unlit --emissive '#ff2000' --double-sided true || fail "the material was not changed"
ctl scene material "$draft" doll paint --shader pbr --param metallic=0.4 --param roughness=0.3 || fail "the material took no numbers"
if ctl scene material "$draft" doll paint --param metallic=3 2>/dev/null; then fail "a metallic of 3 was kept"; fi
if ctl scene material "$draft" doll ghost --lit false 2>/dev/null; then fail "a missing material was changed"; fi
ctl scene material "$draft" doll --list | grep -q '"sphere"' || fail "the materials were not listed with a preview"
ctl scene camera3d "$draft" --fov 45 --eye 0,1,6 --center 0,0,0 || fail "the 3D camera was not set"
if ctl scene camera3d "$draft" --fov 200 2>/dev/null; then fail "a field of view of 200 was kept"; fi
ctl scene camera3d "$draft" --roll -15 || fail "the 3D camera was not rolled"
ctl scene view3d "$draft" --orbit 40,10 --zoom 0.5 || fail "the authoring view did not move"
ctl scene group3d "$draft" show | grep -q '"center": \[' || fail "the camera handles were not shown from the authoring view"
ctl scene view3d "$draft" --frame doll || fail "the authoring view did not frame the model"
ctl scene camera3d "$draft" --from-view || fail "the camera was not taken from the authoring view"
ctl scene scene3d "$draft" --view '{"orbit": [10, 5]}' || fail "a 3D edit given as JSON was not applied"
if ctl scene scene3d "$draft" --view '{}' --tones '{}' 2>/dev/null; then fail "two 3D edits were applied at once"; fi
ctl scene view3d "$draft" --reset || fail "the authoring view was not reset"
ctl scene tonemap "$draft" --hdr true --map aces --split 0.5 || fail "the tone map was not set"
if ctl scene tonemap "$draft" --map filmic 2>/dev/null; then fail "an unknown tone map was kept"; fi
ctl scene cost3d "$draft" --quality q0 | grep -q '"specular": false' || fail "the cost of the models was not shown"
ctl scene lights "$draft" --cost | grep -q '"models"' || fail "the lighting cost does not count the models"
ctl scene validate "$draft" >/dev/null || fail "the 3D scene is not valid"
ctl scene preview "$draft" -o preview:1 --time 0.2 || fail "the 3D scene was not previewed"
ctl scene tonemap "$draft" --hdr false --map none --split 0 || fail "the tone map was not turned off"
ctl scene camera3d "$draft" --clear || fail "the 3D camera was not removed"
ctl scene remove-layer "$draft" world >/dev/null || fail "the 3D group was not removed"
