#!/usr/bin/env bash
set -euo pipefail
declare scene
made=$scene/authoring
ctl scene new "$made" >/dev/null
ctl scene add-layer "$made" solid --id fill >/dev/null
ctl scene duplicate-layer "$made" fill --template || fail "no template was made"
ctl scene duplicate-layer "$made" fill --template || fail "no second template was made"
grep -q '"fill_template_2"' "$made/scene.json" || fail "the second template did not get a free name"
ctl scene import "$made" "$ROOT/tests/fixtures/magenta-64x32.webm" --id clip >/dev/null || fail "the video was not imported"
ctl scene set "$made" /scene/layers/0 '{"id": "film", "type": "video", "video": "clip", "playback": {"own_clock": true, "start": 0.5, "rate": 2}}'
ctl scene clocks "$made" --time 1 | grep '"layer": "film"' >/dev/null || fail "the own clock of the video was not listed"
ctl scene add-layer "$made" particles --id flakes >/dev/null
ctl scene particles "$made" flakes handles --seed 7 --sound 0.5 | grep '"seed": 7' >/dev/null ||
  fail "the rays were not drawn with the seed asked for"
if ctl scene particles "$made" flakes handles --sound 2 2>/dev/null; then fail "a sound level above 1 was taken"; fi
printf '%s' '{"id": "throb", "name": {"en": "Pulse"}, "cost": {"taps": 1}, "passes": [{"entry": "fs_throb"}],
  "params": [{"id": "gain", "type": "float", "default": 1, "min": 0, "max": 4}, {"id": "speed", "type": "float", "default": 0}]}' >"$WORK/throb.json"
printf '%s' '@fragment fn fs_throb(in: VertexOut) -> @location(0) vec4<f32> { return tap(in.uv) * param(0u).x; }' >"$WORK/throb.wgsl"
ctl scene import-effect "$made" "$WORK/throb.json" >/dev/null || fail "the package effect was not imported"
ctl scene effect-rule "$made" throb gain --uses time --waits-for speed || fail "the wake rule was not changed"
ctl scene effect-rule "$made" throb | grep '"waits_for": "speed"' >/dev/null || fail "the wake rules were not printed"
if ctl scene effect-rule "$made" throb gain --waits-for nothing 2>/dev/null; then fail "a rule naming no parameter was kept"; fi
ctl scene effect-rule "$made" throb gain --uses '' --waits-for '' || fail "the wake rule was not cleared"
ctl scene --inside fill_template group "$made" fill --id box || fail "the template layer was not grouped"
ctl scene add-layer "$made" solid --id dot --into box --inside fill_template >/dev/null || fail "no layer was added inside the template"
ctl scene show "$made" /scene/templates/fill_template/layers/1/id | grep '"dot"' >/dev/null ||
  fail "the layer did not land in the template"
if ctl scene --inside fill_template add-layer "$made" solid --id loose 2>/dev/null; then fail "a second root was added to a template"; fi
ctl scene open "$made" --json --inside fill_template | grep -F '"pointer":"/scene/templates/fill_template/layers/1"' >/dev/null ||
  fail "the outline of the template did not list its layers"
ctl scene clocks "$made" --strip 3 | grep -c '"file":' | grep -x 3 >/dev/null || fail "the own clock did not get a strip of three frames"
