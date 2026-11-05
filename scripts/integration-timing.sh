#!/usr/bin/env bash
set -euo pipefail
declare scene ROOT WORK
timed=$scene/timed
ctl scene new "$timed" --id org.example.timed >/dev/null
ctl scene add-layer "$timed" solid --id card >/dev/null
ctl scene add-layer "$timed" mesh --id body --file "$ROOT/tests/fixtures/parts.glb" >/dev/null || fail "no mesh layer came from a .glb file"
if ctl scene add-layer "$timed" mesh --file "$ROOT/tests/fixtures/green-64x32.png" 2>/dev/null; then fail "a picture became a mesh"; fi
ctl scene clip "$timed" body wave --add --play || fail "no clip was added that plays"
[ "$(ctl scene show "$timed" /scene/layers/1/animations/0/clip)" = '"wave"' ] || fail "the new clip does not play"
if ctl scene clip "$timed" body wave --length 1 --play 2>/dev/null; then fail "--play was taken without --add"; fi
ctl scene timeline "$timed" intro --add --duration 4 --loop repeat || fail "no timeline was added"
if ctl scene timeline "$timed" intro --add 2>/dev/null; then fail "a timeline was added twice"; fi
if ctl scene timeline "$timed" intro --fps 900 --frames 10 2>/dev/null; then fail "900 frames a second were kept"; fi
ctl scene keyframe "$timed" /scene/layers/0/opacity 0 1 --timeline intro || fail "no keyframe on the timeline"
ctl scene keyframe "$timed" /scene/layers/0/opacity 3 -0.5 --base 0.75 || fail "the keyframes took no base"
ctl scene keyframe "$timed" /scene/layers/0/opacity 0 --out-handle 1,0 || fail "the keyframe took no handle"
ctl scene keyframe "$timed" /scene/layers/0/opacity 3 --in-handle 1,0 || fail "the keyframe took no handle into it"
if ctl scene keyframe "$timed" /scene/layers/0/opacity 0 --out-handle -1,0 2>/dev/null; then fail "a handle went back in time"; fi
if ctl scene keyframe "$timed" /scene/layers/0/opacity 0 --out-handle 5,0 2>/dev/null; then fail "a handle reached past the next keyframe"; fi
if ctl scene keyframe "$timed" /scene/layers/0/opacity 3 --out-handle 1,0 2>/dev/null; then fail "the last keyframe took a handle out of it"; fi
if ctl scene keyframe "$timed" /scene/layers/0/opacity 0.5 --from 3 2>/dev/null; then fail "a move left a handle past its neighbour"; fi
ctl scene validate "$timed" >/dev/null || fail "keyframes with handles left the scene invalid"
if ctl scene keyframe "$timed" /scene/layers/0/opacity 0 --base '"#ffffff"' 2>/dev/null; then fail "a color was the base of a number"; fi
ctl scene timeline "$timed" intro --rename opening --speed 2 || fail "the timeline was not renamed"
[ "$(ctl scene show "$timed" /scene/layers/0/opacity/timeline)" = '"opening"' ] || fail "the keyframes lost their timeline"
ctl scene preview "$timed" -o preview:1 --time 0.5 || fail "a scene with relative and curved keyframes was not previewed"
ctl scene timeline "$timed" opening --remove || fail "the timeline was not removed"
ctl scene keyframe "$timed" /scene/layers/0/opacity 0 --absolute || fail "the base was not dropped"
printf 'import { scene } from "wprs";\nexport function update() { scene.parallax = { amount: 2 }; }\n' >"$WORK/tilt.js"
ctl scene import-script "$timed" "$WORK/tilt.js" >/dev/null || fail "the camera script was not imported"
ctl scene show "$timed" /scene/scripts | grep -q '"camera"' || fail "a script that moves the parallax does not declare camera"
