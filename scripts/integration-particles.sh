#!/usr/bin/env bash
set -euo pipefail
declare draft
ctl scene add-layer "$draft" particles --id dust >/dev/null || fail "no particles layer was added"
ctl scene particles "$draft" dust control-point p1 --at 40,30 --angle 15 || fail "no control point was placed"
ctl scene particles "$draft" dust shape 0 '{"around": {"at": "p1", "count": 12, "radius": 20, "mode": "mirror"}}' ||
  fail "the emitter was not put around a point"
ctl scene particles "$draft" dust shape 1 '{"between": {"from": "p0", "to": "p1", "count": 8, "arc": {"amount": 0.2}}}' ||
  fail "no emitter between two points was added"
ctl scene particles "$draft" dust emit-from-layer 1 badge --threshold 0.4 --color --inherit --refresh on_change ||
  fail "the emitter does not take a layer"
if ctl scene particles "$draft" dust emit-from-layer 0 dust 2>/dev/null; then fail "a layer fed its own particles"; fi
ctl scene particles "$draft" dust remap add '{"input": "random", "in": [0, 1], "output": "size", "out": [2, 6]}' ||
  fail "no remap was added"
ctl scene particles "$draft" dust remap set 0 '{"input": "distance:p1", "in": [0, 100], "output": "alpha", "out": [1, 0]}' ||
  fail "the remap was not set"
ctl scene particles "$draft" dust remap add '{"input": "daytime", "in": [0, 1], "output": "spin", "out": [0, 90]}'
ctl scene particles "$draft" dust remap remove 1 || fail "the remap was not removed"
if ctl scene particles "$draft" dust remap add '{"input": "weather", "in": [0, 1], "output": "size", "out": [0, 1]}' 2>/dev/null; then
  fail "a remap of an unknown input was kept"
fi
ctl scene particles "$draft" dust cutout 0.3,0.7,0.9 || fail "the alpha was not cut out"
ctl scene particles "$draft" dust orient fixed --angle 30 || fail "the sprites were not turned"
ctl scene particles "$draft" dust perspective --focal 600 --depth 0,300 --depth-speed -10,10 || fail "no depth was given"
ctl scene set "$draft" /scene/particles/spark '{"capacity": 10, "emitters": [{"shape": {"point": [0, 0]}}]}'
ctl scene set "$draft" /scene/particles/dust/children '[{"system": "spark", "on": "follow"}]'
ctl scene particles "$draft" dust inherit 0 color size --follow || fail "the child took nothing from its parent"
ctl scene particles "$draft" dust shader none || fail "the shader was not cleared"
ctl scene particles "$draft" dust handles | grep -q '"around"' || fail "the handles of the system were not listed"
ctl scene show "$draft" /manifest/requires/features | grep -q particle_depth || fail "depth left particle_depth out"
ctl scene preview "$draft" -o preview:1 --time 0.5 || fail "a scene with newer particles was not previewed"
ctl scene particles "$draft" dust perspective --off || fail "the depth was not removed"
