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
