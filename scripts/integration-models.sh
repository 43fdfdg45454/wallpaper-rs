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
rig=$ROOT/tests/fixtures/rig.glb
ctl scene import "$draft" "$rig" --id rig3d >/dev/null || fail "no rigged model was imported"
ctl scene model "$draft" add walker --asset rig3d --into world || fail "no rigged model layer was added"
ctl scene animate "$draft" walker bones | grep -q '"tip"' || fail "the bones of the model were not listed"
ctl scene clip "$draft" walker bend --add || fail "no clip was added to the model"
ctl scene clip "$draft" walker bend --animation bend --from 0.5 || fail "the clip did not take the animation of the file"
if ctl scene clip "$draft" walker bend --animation nothing 2>/dev/null; then fail "a missing animation was played"; fi
ctl scene key "$draft" walker bend 0.5 --bone tip --angles 0,0,30 || fail "no 3D key was set"
ctl scene animate "$draft" walker layer --add bend || fail "no animation layer was added"
ctl scene animate "$draft" walker layer --index 0 --weight 0.5 --rate 2 --blend-in 0.3 --bones tip || fail "the animation layer was not tuned"
if ctl scene animate "$draft" walker layer --index 0 --weight 3 2>/dev/null; then fail "a weight of 3 was kept"; fi
ctl scene animate "$draft" walker clips | grep -q '"files"' || fail "the clips of the model were not listed"
ctl scene animate "$draft" walker root true || fail "root motion was not turned on"
ctl scene animate "$draft" walker preset tip hair || fail "the physics preset was not applied"
ctl scene animate "$draft" walker morph bulge 0.6 || fail "the blend shape was not weighed"
if ctl scene animate "$draft" walker morph nothing 0.5 2>/dev/null; then fail "a missing blend shape was weighed"; fi
ctl scene group3d "$draft" show | grep -q '"bones"' || fail "the bones of the model were not shown in the authoring view"
ctl scene material "$draft" doll paint --shader fur --param layers=12 --param length=20 --param comb_x=0.4 || fail "no fur"
ctl scene material "$draft" walker body --shader chroma --key '#00ff00' --param threshold=0.3 || fail "no chroma key"
if ctl scene material "$draft" doll paint --param layers=99 2>/dev/null; then fail "99 fur shells were kept"; fi
ctl scene new-material "$draft" gloss_coat || fail "no material shader was started"
ctl scene new-material "$draft" gloss_coat --check | grep -q '"ok": true' || fail "the new material shader does not build"
ctl scene material "$draft" doll paint --shader gloss_coat || fail "the package material was not picked"
ctl scene add-layer "$draft" light --id sun >/dev/null || fail "no light was added"
ctl scene shadows "$draft" sun --on true --resolution 1024 --bias 0.3 || fail "the light casts no shadows"
if ctl scene shadows "$draft" sun --resolution 100 2>/dev/null; then fail "a shadow map of 100 pixels was kept"; fi
ctl scene shadows "$draft" sun --show | grep -q '"tiers"' || fail "the shadow maps per tier were not shown"
ctl scene debug3d "$draft" shadows || fail "the 3D debug view did not show shadows"
ctl scene debug3d "$draft" none || fail "the 3D debug view did not go back to the light"
ctl scene camera-path "$draft" add tour || fail "no camera path was added"
ctl scene camera-path "$draft" key tour 4 --eye 2,1,6 --center 0,0,0 --fov 40 || fail "no path key was set"
ctl scene camera-path "$draft" set tour --loop true --ease in_out_sine || fail "the path does not loop"
ctl scene camera-path "$draft" order random || fail "the paths were not ordered"
ctl scene camera-path "$draft" fade --to cross --duration 0.5 || fail "the paths were not faded"
if ctl scene camera-path "$draft" key tour 1 --fov 300 2>/dev/null; then fail "a key of 300 degrees was kept"; fi
ctl scene camera-path "$draft" show --time 1 | grep -q '"spline"' || fail "the path spline was not shown"
ctl scene cost3d "$draft" --quality q3 | grep -q '"fur_shells"' || fail "the cost does not count fur shells"
ctl scene validate "$draft" >/dev/null || fail "the animated 3D scene is not valid"
ctl scene preview "$draft" -o preview:1 --time 0.5 || fail "the animated 3D scene was not previewed"
ctl scene camera-path "$draft" remove tour || fail "the path was not removed"
ctl scene camera3d "$draft" --clear || fail "the 3D camera was not removed"
ctl scene remove-layer "$draft" world >/dev/null || fail "the 3D group was not removed"
