#!/usr/bin/env bash
set -euo pipefail
declare scene
draft=$scene/draft
ctl scene new "$draft" >/dev/null
ctl scene set "$draft" /scene/canvas/background '#00ff00'
read -r _ _ _ s15 < <(ctl stream preview:1 160 90)
expect "list(s['outputs'])" "[]"
ctl scene preview "$draft" -o preview:1 --time 0
shown "$s15" 0 255 0
ctl scene add-layer "$draft" solid >/dev/null
ctl scene set "$draft" /scene/layers/0/color '#ff0000'
ctl scene preview "$draft" -o preview:1 --time 0
shown "$s15" 255 0 0
ctl scene solo "$draft" solid && ctl scene hide "$draft" solid
ctl scene preview "$draft" -o preview:1 --time 2
shown "$s15" 0 255 0
if ctl scene set "$draft" /scene/canvas/size/0 0 && ctl scene preview "$draft" -o preview:1 2>/dev/null; then
  fail "an invalid scene was previewed"
fi
ctl scene undo "$draft"
ctl scene set "$draft" /scene/layers/0/blend linear_burn
ctl scene preview "$draft" -o preview:1 --time 2 || fail "a scene with a newer blend mode was not previewed"
ctl scene undo "$draft"
ctl scene set "$draft" /scene/canvas/hdr true
if refused=$(ctl scene preview "$draft" -o preview:1 2>&1); then fail "a scene with an hdr canvas was previewed"; fi
grep -q "hdr" <<<"$refused" || fail "the refused preview did not name its feature: $refused"
ctl scene undo "$draft"
ctl scene set "$draft" /scene/layers/0/transform/tilt '[10, 0]'
ctl scene set "$draft" /scene/canvas/camera '{"zoom": 1.5, "rotation": 10}'
ctl scene set "$draft" /scene/canvas/bloom '{"tint": "#ffd0a0"}'
ctl scene preview "$draft" -o preview:1 --time 2 || fail "a scene with a camera, bloom and tilted layers was not previewed"
ctl scene undo "$draft" && ctl scene undo "$draft" && ctl scene undo "$draft"
ctl scene set "$draft" /scene/layers/0/solid '{"alpha": 0.5}'
ctl scene preview "$draft" -o preview:1 --time 2 || fail "a scene with solid layers was not previewed"
ctl scene undo "$draft"
ctl scene set "$draft" /scene/layers/0/effects '[{"effect": "glitter", "params": {"speed": 0}}, {"effect": "fluid", "params": {"source_strength": 8}}, {"effect": "god_rays", "params": {"mode": 2}}]'
ctl scene preview "$draft" -o preview:1 --time 2 || fail "a scene with glitter, fluid and a star of rays was not previewed"
ctl scene undo "$draft"
ctl scene set "$draft" /scene/layers/0/effects '[{"effect": "blend_gradient"}]'
ctl scene paint-mask "$draft" /scene/layers/0/effects/0/params/map '0.4,0.5;0.6,0.5' --channel g || fail "a mask was not painted"
ctl scene preview "$draft" -o preview:1 --time 2 || fail "a scene with a painted mask was not previewed"
ctl scene undo "$draft"
ctl scene undo "$draft"
ctl scene set "$draft" /scene/layers/0/effects '[{"effect": "light_shafts", "params": {"speed": 0}}, {"effect": "tint", "mask": {"shape": {"ellipse": {"size": [40, 40]}}}}]'
ctl scene preview "$draft" -o preview:1 --time 2 || fail "a scene with the newer built-in effects was not previewed"
ctl scene undo "$draft"
ctl scene set "$draft" /scene/clock '{"seed": null}'
ctl scene preview "$draft" -o preview:1 --time 2 || fail "a scene with a fresh seed was not previewed"
ctl scene undo "$draft"
steps=$(ctl scene history "$draft" | grep -vc '(undone)$')
ctl scene import "$draft" "$ROOT/tests/fixtures/green-64x32.png" --id leaf >/dev/null
ctl scene mesh-from-picture "$draft" leaf --cells 8 --id puppet >/dev/null || fail "no mesh was cut from a picture"
ctl scene bone "$draft" puppet root --at 0.2,0.5 || fail "the mesh took no root bone"
ctl scene bone "$draft" puppet arm --parent root --at 0.6,0.5 || fail "the mesh took no arm"
ctl scene bones "$draft" puppet | grep -q '"arm"' || fail "the bones of the mesh were not listed"
ctl scene set "$draft" /scene/layers/1/clips '[{"id": "wave", "bones": {"arm": {"rotation": [[0, 0], [1, 40]]}}}]'
ctl scene set "$draft" /scene/layers/1/animations '[{"clip": "wave"}]'
ctl scene preview "$draft" -o preview:1 --time 0.5 || fail "a scene with a bent mesh was not previewed"
ctl scene set "$draft" /scene/layers/1/ik '[{"chain": ["root", "arm"], "target": "cursor", "limits": {"arm": [-60, 60]}}]'
ctl scene set "$draft" /scene/layers/1/physics '[{"bone": "arm", "mode": "rope", "gravity": 1, "wind": 0.5}]'
ctl scene set "$draft" /scene/layers/1/depth '{"scale": 1, "cull": true}'
ctl scene mesh-depth "$draft" puppet '0.4,0.5;0.6,0.5' --by 30 --radius 0.2 || fail "the depth of the mesh was not painted"
ctl scene set "$draft" /scene/layers/1/hit '["arm"]'
ctl scene set "$draft" /scene/canvas/wind '[40, 0]'
ctl scene preview "$draft" -o preview:1 --time 0.5 || fail "a scene with reaching and swinging bones was not previewed"
ctl scene bone "$draft" puppet hand --parent arm --at 0.9,0.5 || fail "the mesh took no hand"
ctl scene weights "$draft" puppet arm '0.5,0.5;0.6,0.5' --mode add --by 0.5 --radius 0.2 --lock root || fail "no weights were painted"
ctl scene vertex-weights "$draft" puppet | grep -q '"arm"' || fail "the weights of the vertices were not listed"
ctl scene bone "$draft" puppet hand --rename paw || fail "the hand was not renamed"
ctl scene bone "$draft" puppet paw --root || fail "the paw was not rehung"
ctl scene auto-weights "$draft" puppet || fail "the weights were not bound to the bones again"
ctl scene clip "$draft" puppet swing --add --length 2 --loop pingpong || fail "no clip was added"
ctl scene key "$draft" puppet swing 1 --bone arm --rotation 30 --position 4,0 --scale 1.2 --ease cubic || fail "the arm was not keyed"
ctl scene key "$draft" puppet swing 1.5 --bone arm --from 1 --copy || fail "the key was not copied"
ctl scene key "$draft" puppet swing 0 --order body --value 3 || fail "the order of a part was not keyed"
ctl scene keys "$draft" puppet swing | grep -q '"cubic"' || fail "the keys of the clip were not listed"
ctl scene clip "$draft" puppet swing --rename sway || fail "the clip was not renamed"
ctl scene set "$draft" /scene/layers/1/animations '[{"clip": "sway"}]'
ctl scene preview "$draft" -o preview:1 --time 0.5 || fail "a scene with a keyed and painted mesh was not previewed"
ctl scene ik "$draft" puppet add root arm --limit arm -45 45 --bend counterclockwise || fail "no chain was added"
ctl scene ik "$draft" puppet set 1 --target 30,20 --weight 0.8 || fail "the chain was not aimed"
ctl scene ik "$draft" puppet remove 0 || fail "the chain was not removed"
ctl scene physics "$draft" puppet add arm --mode spring --stiffness 0.3 --gravity -2 || fail "the arm does not swing"
ctl scene physics "$draft" puppet rope tail '0.6,0.6;0.7,0.7;0.8,0.8' --parent arm --wind 0.5 || fail "no rope was drawn"
ctl scene physics "$draft" puppet add arm --rotation -20,20 --translation -4,4 || fail "the swing of the arm was not bounded"
ctl scene physics "$draft" puppet add arm --free rotation || fail "the bound of the arm was not lifted"
ctl scene physics "$draft" puppet remove arm || fail "the arm still swings"
ctl scene wind "$draft" --angle 30 --strength 60 --noise 0.5 || fail "the wind was not set"
ctl scene add-layer "$draft" solid --id badge >/dev/null
ctl scene attach "$draft" badge --to puppet --bone arm --inherit position,rotation || fail "the layer was not attached"
ctl scene attach "$draft" badge --point 0.5,0.4 || fail "the layer was not pinned to a new point"
ctl scene attach "$draft" badge --nudge 4,-2 || fail "the layer was not moved along its point"
ctl scene ik "$draft" puppet add root arm || fail "no chain without limits was added"
ctl scene point "$draft" puppet pin --at 0.5,0.3 || fail "no point was placed"
ctl scene point "$draft" puppet pin --rename peg --parent root || fail "the point was not renamed and rehung"
ctl scene bones "$draft" puppet | grep -q '"peg"' || fail "the point was not listed"
ctl scene point "$draft" puppet peg --remove || fail "the point was not removed"
ctl scene validate "$draft" >/dev/null || fail "removing a point left the scene invalid"
ctl scene parts "$draft" puppet --order body 2 --hide body || fail "the part was not ordered"
ctl scene parts "$draft" puppet --show body || fail "the part was not shown"
ctl scene preview "$draft" -o preview:1 --time 0.5 || fail "a scene with chains, a rope, wind and attachments was not previewed"
ctl scene attach "$draft" badge --detach || fail "the layer was not let go"
ctl scene bone "$draft" puppet tip --parent arm --at 0.9,0.5 || fail "the mesh took no tip"
ctl scene ik "$draft" puppet add arm tip --limit tip -30 30 || fail "no chain to the tip was added"
ctl scene physics "$draft" puppet add tip || fail "the tip does not swing"
ctl scene key "$draft" puppet sway 0.5 --bone tip --rotation 10 || fail "the tip was not keyed"
ctl scene bone "$draft" puppet tip --remove || fail "the tip was not removed"
ctl scene validate "$draft" >/dev/null || fail "removing a bone left the scene invalid"
ctl scene undo "$draft"
ctl scene bone "$draft" puppet arm --root || fail "the arm was not rehung"
ctl scene validate "$draft" >/dev/null || fail "rehanging a bone left the scene invalid"
ctl scene undo "$draft"
ctl scene bone "$draft" puppet arm --rename limb --parent paw || fail "the arm was not renamed and rehung"
ctl scene validate "$draft" >/dev/null || fail "renaming and rehanging a bone left the scene invalid"
ctl scene import "$draft" "$ROOT/tests/fixtures/parts.glb" --id body >/dev/null
ctl scene add "$draft" /scene/layers/- '{"id": "body", "type": "mesh", "mesh": "body", "size": [100, 100]}'
ctl scene bone "$draft" body tail --parent hip --at 0.8,0.8 || fail "a mesh of several parts took no bone"
ctl scene expression "$draft" body happy --shape smile=0.8 || fail "no expression was made"
ctl scene expression "$draft" body happy --rename glad || fail "the expression was not renamed"
ctl scene clip "$draft" body grin --add || fail "no clip was added to the mesh of several parts"
ctl scene key "$draft" body grin 0.5 --shape smile --weight 0.5 || fail "the blend shape was not keyed"
ctl scene parts "$draft" body --clip chest legs --order legs -1 || fail "the part was not cut to another"
ctl scene preview "$draft" -o preview:1 --time 0.5 || fail "a scene with a mesh of several parts was not previewed"
ctl scene build "$draft" -o "$WORK/meshes.wpak" >/dev/null || fail "the scene with meshes was not built"
packed=$(python3 -c "import sys, zipfile; print(sum(n.startswith('meshes/') for n in zipfile.ZipFile(sys.argv[1]).namelist()))" "$WORK/meshes.wpak")
[ "$packed" = 2 ] || fail "the package carries $packed meshes instead of the two in use"
while [ "$(ctl scene history "$draft" | grep -vc '(undone)$')" -gt "$steps" ]; do ctl scene undo "$draft"; done
"$BIN/probe" "$s15" 4 --hold >"$WORK/edit-frames.out" 2>&1 & counter=$!; pids+=("$counter")
sleep 1
ctl scene set "$draft" /scene/canvas/background '#0000ff'
ctl scene preview "$draft" -o preview:1 --time 2
wait "$counter" || fail "no frames while editing: $(cat "$WORK/edit-frames.out")"
frames=$(sed -n 's/^frames //p' "$WORK/edit-frames.out")
[ "$frames" -lt 15 ] || fail "one edit of a held preview rendered $frames frames"
ctl scene solo "$draft" && ctl scene hide "$draft" solid --hidden false
ctl scene keyframe "$draft" /scene/layers/0/color 0 '#ff0000'
ctl scene keyframe "$draft" /scene/layers/0/color 16 '#0000ff'
"$BIN/probe" "$s15" 10 --hold >"$WORK/play-frames.out" 2>&1 & player=$!; pids+=("$player")
ctl scene preview "$draft" -o preview:1
sleep 6
ctl scene set "$draft" /scene/canvas/background '#00ff00'
ctl scene preview "$draft" -o preview:1
read -r b g r _ < <("$BIN/probe" "$s15" 3 | sed 's/.*center bgra \[\(.*\)\]/\1/; s/,/ /g')
[ "$b" -gt 85 ] || fail "an edit restarted the playing preview: bgra $b $g $r"
wait "$player" || fail "the playing preview stopped: $(cat "$WORK/play-frames.out")"
ctl scene translate "$draft" /manifest/title es Borrador
ctl scene set "$draft" /manifest/content '{"rating": "mature", "descriptors": ["gore"]}'
ctl scene preview "$draft" -o preview:1 --time 0
shown "$s15" 255 0 0
ctl scene remove "$draft" /manifest/content && ctl scene remove "$draft" /manifest/origin
ctl scene hide "$draft" solid
ctl scene preview "$draft" -o preview:1 --time 0 || fail "incomplete package details stopped the preview"
shown "$s15" 0 255 0
ctl scene hide "$draft" solid --hidden false
[ "$(ctl scene add-property "$draft" combo --id look)" = look ] || fail "add-property printed no id"
ctl scene set "$draft" /manifest/properties/look/options '[{"value": "red", "label": "Red"}, {"value": "blue", "label": "Blue"}]'
ctl scene set "$draft" /manifest/properties/look/default blue
ctl scene set "$draft" /scene/layers/0/color '{"prop": "look", "map": {"red": "#ff0000", "blue": "#0000ff"}}'
ctl scene add-property "$draft" button --id flash >/dev/null
if ctl scene condition "$draft" flash "look > 2" 2>/dev/null; then fail "a condition that compares a text with a number was kept"; fi
ctl scene condition "$draft" flash "look == 'red'" --enabled
ctl scene rename-property "$draft" look tone
ctl scene preview "$draft" -o preview:1 --time 0
shown "$s15" 0 0 255
ctl scene set "$draft" /manifest/properties/tone/default red
ctl scene preview "$draft" -o preview:1 --time 0
shown "$s15" 255 0 0
ctl scene show "$draft" /manifest/requires/features | grep -q properties2 || fail "a button left properties2 out of the features"
if ctl scene remove-property "$draft" tone 2>/dev/null; then fail "a property that a binding follows was removed"; fi
ctl scene remove-property "$draft" flash
if ctl scene show "$draft" /manifest/properties | grep -q flash; then fail "remove-property kept the button"; fi
ctl scene preview "$draft" -o preview:1 --stop
ctl release preview:1 && expect "list(s['outputs'])" "[]"
echo "scene editor preview checks passed"
sea=org.example.sea
inputs() { ctl input status --json | python3 -c "import json, sys; r = json.load(sys.stdin); print(eval(sys.argv[1]))" "$1"; }
[ "$(inputs "r['grants'], r['capturing'], r['tracking']")" = "({}, False, False)" ] || fail "inputs start on"
if ctl input grant "$sea" audio 2>"$WORK/grant.err"; then fail "an input was granted to a scene that is not shown"; fi
grep -q "is not shown" "$WORK/grant.err" || fail "grant before showing: $(cat "$WORK/grant.err")"
ctl input pointer 10 10 && ctl input pointer -10 -10
ctl input status | grep -qx "audio capture: off" || fail "status: $(ctl input status)"
refused "is not audio, pointer, clicks or network" Inputs1 Grant ss "$sea" camera
refused "is not a scene package id" Inputs1 Grant ss "Not A Scene" audio
refused "Signature mismatch" Inputs1 SetPointer dd 1 1
denied "only the KWin script of wprsd and wprs-ctl report the pointer" Inputs1.SetPointer int32:1 int32:1
echo "input permission checks passed"
