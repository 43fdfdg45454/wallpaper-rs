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
ctl scene set "$draft" /scene/layers/0/transform/tilt '[10, 0]'
if refused=$(ctl scene preview "$draft" -o preview:1 2>&1); then fail "a scene with tilted layers was previewed"; fi
grep -q "tilt" <<<"$refused" || fail "the refused preview did not name its feature: $refused"
ctl scene undo "$draft"
ctl scene set "$draft" /scene/layers/0/solid '{"alpha": 0.5}'
ctl scene preview "$draft" -o preview:1 --time 2 || fail "a scene with solid layers was not previewed"
ctl scene undo "$draft"
ctl scene set "$draft" /scene/layers/0/effects '[{"effect": "glitter"}]'
if refused=$(ctl scene preview "$draft" -o preview:1 2>&1); then fail "a scene with an effect not drawn yet was previewed"; fi
grep -q "effect.glitter" <<<"$refused" || fail "the refused preview did not name effect.glitter: $refused"
ctl scene undo "$draft"
ctl scene set "$draft" /scene/layers/0/effects '[{"effect": "light_shafts", "params": {"speed": 0}}, {"effect": "tint", "mask": {"shape": {"ellipse": {"size": [40, 40]}}}}]'
ctl scene preview "$draft" -o preview:1 --time 2 || fail "a scene with the newer built-in effects was not previewed"
ctl scene undo "$draft"
ctl scene set "$draft" /scene/clock '{"seed": null}'
ctl scene preview "$draft" -o preview:1 --time 2 || fail "a scene with a fresh seed was not previewed"
ctl scene undo "$draft"
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
