#!/usr/bin/env bash
set -euo pipefail
declare config log morning profile
hold=$XDG_DATA_HOME/wallpaper-rs/profile-hold.json
ctl stream TEST-8 160 90 >/dev/null
ctl profile use Morning && expect "s['profile']" "$morning"
ctl release TEST-8 && expect "list(s['outputs'])" "[]"
kill "$daemon"
wait "$daemon" 2>/dev/null || true
RUST_LOG=$log "$BIN/wprsd" >"$WORK/restarted.log" 2>&1 & daemon=$!; pids+=("$daemon")
wait_for 30 ctl state || fail "the daemon did not start again"
ctl stream TEST-8 160 90 >/dev/null
sleep 4
expect "s['profile']" "$morning"
[ -e "$hold" ] || fail "the manual profile choice was forgotten across a restart"
ctl stream preview:9 160 90 >/dev/null
sleep 3
ctl release preview:9
sleep 3
expect "s['profile']" "$morning"
[ -e "$hold" ] || fail "a preview stream replaced the profile chosen by hand"
ctl stream TEST-9 160 90 >/dev/null
wait_for 30 test ! -e "$hold" || fail "the manual profile choice outlived a change of outputs"
ctl release TEST-9
evening() { [ "$(field "s['profile']")" = "$profile" ]; }
wait_for 30 evening || fail "the profile of the outputs was not activated: $(ctl state)"
ctl release TEST-8 && expect "list(s['outputs'])" "[]"
echo "manual profile choice kept across a restart"
scene=$WORK/scene
mkdir -p "$scene"
python3 - "$scene/glow.wpak" "$ROOT/tests/fixtures/green-64x32.png" <<'PACKAGE'
import json, sys, zipfile
manifest = {"schema": "wallpaper-rs.wpak/1", "id": "org.example.glow", "version": "1.0.0", "title": "Glow",
            "license": "CC0-1.0", "kind": "scene", "entry": "scene.json", "preview": "preview.png",
            "properties": {"tint": {"type": "color", "label": "Tint", "default": "#0000ff"}},
            "presets": [{"id": "warm", "values": {"tint": "#ff0000"}}]}
scene = {"schema": "wallpaper-rs.scene/1", "canvas": {"size": [64, 32], "fit": "stretch"},
         "assets": {"backdrop": {"path": "assets/backdrop.png"}},
         "layers": [{"id": "backdrop", "type": "image", "image": "backdrop"},
                    {"id": "tint", "type": "solid", "color": {"prop": "tint"}, "size": [32, 16]}]}
with zipfile.ZipFile(sys.argv[1], "w") as package:
    package.writestr("manifest.json", json.dumps(manifest))
    package.writestr("scene.json", json.dumps(scene))
    package.write(sys.argv[2], "preview.png")
    package.write(sys.argv[2], "assets/backdrop.png")
PACKAGE
[ "$(ctl scene bench "$scene/glow.wpak" --frames 20 --size 160x90 --json | js "r['needed'], r['created_after_warmup']")" = "(1, 0)" ] ||
  fail "scene bench: $(ctl scene bench "$scene/glow.wpak" --frames 20 --size 160x90)"
read -r _ _ _ s11 < <(ctl stream TEST-11 160 90)
job=$(bus Library1 Import asb 1 "$scene/glow.wpak" false | out "d[0]")
wait_for 20 finished "$job" || fail "scene import: $(bus Daemon1 GetJob t "$job")"
glow=$(reply "r['job']['output']['imported'][0]['uid']" Daemon1 GetJob t "$job")
[ -s "$(bus Library1 Thumbnail su "$glow" 256 | out "d[0]")" ] || fail "no thumbnail from the scene preview"
ctl lib apply "$glow" -o TEST-11
expect "s['outputs']['TEST-11']['kind'], s['outputs']['TEST-11']['item']" "('scene', '$glow')"
shown "$s11" 0 0 255
ctl prop set tint '#ff0000' -o TEST-11
shown "$s11" 255 0 0
[ "$(ctl prop get tint -o TEST-11)" = '"#ff0000"' ] || fail "property value: $(ctl prop ls -o TEST-11)"
[ "$(ctl prop ls -o TEST-11 --json | js "r['package'], list(r['properties']), r['presets'][0]['id']")" = \
  "('org.example.glow', ['tint'], 'warm')" ] || fail "properties: $(ctl prop ls -o TEST-11 --json)"
grep -q '^\[default.scenes."org.example.glow"\]' "$config" || fail "scene values: $(cat "$config")"
if ctl prop set tint red -o TEST-11 2>/dev/null; then fail "an invalid color was accepted"; fi
ctl prop reset -o TEST-11
shown "$s11" 0 0 255
ctl prop preset load warm -o TEST-11
shown "$s11" 255 0 0
sleep 1
idle=$(ticks) && sleep 2 && idle=$(($(ticks) - idle))
[ "$idle" -lt 50 ] || fail "the daemon used $idle ticks in 2 s while a still scene was shown"
python3 - "$scene/clip.wpak" "$ROOT/tests/fixtures/magenta-64x32.webm" "$ROOT/tests/fixtures/green-64x32.png" <<'PACKAGE'
import json, sys, zipfile
manifest = {"schema": "wallpaper-rs.wpak/1", "id": "org.example.clip", "version": "1.0.0", "title": "Clip",
            "license": "CC0-1.0", "kind": "scene", "entry": "scene.json", "preview": "preview.png"}
scene = {"schema": "wallpaper-rs.scene/1", "canvas": {"size": [64, 32], "fit": "stretch"},
         "assets": {"clip": {"path": "assets/clip.webm"}},
         "layers": [{"id": "clip", "type": "video", "video": "clip", "size": [64, 32]}]}
with zipfile.ZipFile(sys.argv[1], "w") as package:
    package.writestr("manifest.json", json.dumps(manifest))
    package.writestr("scene.json", json.dumps(scene))
    package.write(sys.argv[2], "assets/clip.webm", zipfile.ZIP_STORED)
    package.write(sys.argv[3], "preview.png")
PACKAGE
job=$(bus Library1 Import asb 1 "$scene/clip.wpak" false | out "d[0]")
wait_for 20 finished "$job" || fail "video scene import: $(bus Daemon1 GetJob t "$job")"
clip=$(reply "r['job']['output']['imported'][0]['uid']" Daemon1 GetJob t "$job")
ctl lib apply "$clip" -o TEST-11
shown "$s11" 208 32 96
python3 - "$scene" "$ROOT/tests/fixtures/green-64x32.png" "$ROOT/tests/fixtures/quadrants-64x32.webm" <<'PACKAGE'
import json, sys, zipfile
common = {"schema": "wallpaper-rs.wpak/2", "version": "1.0.0", "license": "CC0-1.0", "photosensitive": False,
          "content": {"rating": "general"}, "origin": {"kind": "original"}, "previews": {"thumbnail": "preview.png"}}
knobs = dict(common, id="org.example.knobs", title="Knobs", kind="scene", entry="scene.json",
             requires={"features": ["properties2", "user_videos"]}, properties={
    "mood": {"type": "combo", "label": "Mood", "default": 1, "options": [{"value": 1, "label": "Calm"}, {"value": 2, "label": "Storm"}]},
    "tint": {"type": "color", "label": "Tint", "default": "#0000ff", "visible_if": "mood == 1"},
    "flash": {"type": "button", "label": "Flash", "enabled_if": "mood == 2"},
    "key": {"type": "text", "label": "Key", "secret": True},
    "photo": {"type": "file", "label": "Photo", "accept": ["image", "video"], "default_asset": "backdrop"}},
    presets=[{"id": "warm", "label": "Warm", "values": {"tint": "#ff0000"}}])
scene = {"schema": "wallpaper-rs.scene/1", "canvas": {"size": [64, 32], "fit": "stretch"},
         "assets": {"backdrop": {"path": "assets/backdrop.png"}},
         "timelines": [{"id": "flash", "duration": 2, "autoplay": False, "trigger": "prop:flash"}],
         "layers": [{"id": "backdrop", "type": "image", "image": {"prop": "photo"}},
                    {"id": "tint", "type": "solid", "color": {"prop": "tint"}, "size": [32, 16],
                     "opacity": {"prop": "mood", "map": {"1": 1, "2": 0}}},
                    {"id": "flash", "type": "solid", "color": "#ffffff", "size": [64, 32],
                     "visible": {"timeline": "flash", "keys": [[0, False], [0.05, True], [2, False]]}}]}
reel = dict(common, id="org.example.reel", title="Reel", kind="video", entry="loop.webm",
            requires={"features": ["video_properties", "accent"]}, playback={"speed": 2, "start": 0.6, "volume": 0.5},
            properties={"mark": {"type": "color", "label": "Mark", "default": "#123456"}},
            display={"fit": "stretch", "zoom": 2, "position": [0.5, 0.5], "accent": {"prop": "mark"}},
            presets=[{"id": "slow", "values": {"speed": 0.5}}])
for name, manifest, files in [("knobs", knobs, {"scene.json": json.dumps(scene)}), ("reel", reel, {})]:
    with zipfile.ZipFile(f"{sys.argv[1]}/{name}.wpak", "w") as package:
        package.writestr("manifest.json", json.dumps(manifest))
        [package.writestr(k, v) for k, v in files.items()]
        package.write(sys.argv[2], "preview.png")
        package.write(sys.argv[2], "assets/backdrop.png")
        package.write(sys.argv[3], "loop.webm", zipfile.ZIP_STORED)
PACKAGE
imported() {
  local job; job=$(bus Library1 Import asb 1 "$1" false | out "d[0]")
  wait_for 20 finished "$job" || fail "import of $1: $(bus Daemon1 GetJob t "$job")"
  reply "r['job']['output']['imported'][0]['uid']" Daemon1 GetJob t "$job"
}
knobs=$(imported "$scene/knobs.wpak") && reel=$(imported "$scene/reel.wpak")
ctl lib apply "$knobs" -o TEST-11
shown "$s11" 0 0 255
settings() { ctl prop ls -o TEST-11 --json | js "$1"; }
[ "$(settings "[(f['key'], f['visible'], f['enabled']) for f in r['fields'] if f['key'] in ('tint', 'flash')]")" = \
  "[('flash', True, False), ('tint', True, True)]" ] || fail "conditions: $(ctl prop ls -o TEST-11)"
if ctl prop press flash -o TEST-11 2>/dev/null; then fail "a button was pressed while it was off"; fi
ctl prop set mood 2 -o TEST-11 && shown "$s11" 0 255 0
ctl prop press flash -o TEST-11 && shown "$s11" 255 255 255
png "$scene/red.png" ff0000
(cd "$scene" && ctl prop set photo red.png -o TEST-11) && shown "$s11" 255 0 0
[ "$(ctl prop get photo -o TEST-11)" = "\"$(realpath "$scene/red.png")\"" ] || fail "a relative path was kept: $(ctl prop ls -o TEST-11)"
(cd "$ROOT/tests" && ctl prop set photo fixtures/magenta-64x32.webm -o TEST-11) && shown "$s11" 208 32 96
(cd "$scene" && cp red.png 2024 && ctl prop set photo 2024 -o TEST-11) && shown "$s11" 255 0 0
(cd "$ROOT/tests" && ctl prop set photo '"fixtures/magenta-64x32.webm"' -o TEST-11) && shown "$s11" 208 32 96
ctl prop set photo null -o TEST-11 && shown "$s11" 0 255 0
if printf 'hunter2\n' | ctl prop set key -o TEST-11 2>"$WORK/key.err"; then fail "a secret was stored without a keyring"; fi
grep -q "no Secret Service is available" "$WORK/key.err" || fail "secret message: $(cat "$WORK/key.err")"
ctl prop set tint '#00ff00' -o TEST-11 && ctl prop preset save Mine -o TEST-11
ctl prop reset -o TEST-11 && ctl prop preset load mine -o TEST-11
[ "$(ctl prop get tint -o TEST-11)" = '"#00ff00"' ] || fail "the saved preset was not loaded: $(ctl prop ls -o TEST-11)"
ctl prop preset rename Mine Leaf -o TEST-11 && ctl prop preset export Leaf -f "$scene/leaf.json" -o TEST-11
[ "$(js "r['format'], r['package'], r['values']['mood'], 'photo' in r['values']" <"$scene/leaf.json")" = \
  "('wpak-values/1', 'org.example.knobs', 2, False)" ] || fail "exported values: $(cat "$scene/leaf.json")"
ctl prop preset rm Leaf -o TEST-11 && ctl prop reset -o TEST-11
ctl prop preset import "$scene/leaf.json" --save Again -o TEST-11
[ "$(ctl prop preset ls -o TEST-11 --json | js "[p['label'] for p in r['user_presets']]")" = "['Again']" ] ||
  fail "imported preset: $(ctl prop preset ls -o TEST-11)"
ctl prop preset import - -o TEST-11 <"$scene/leaf.json"
[ "$(ctl prop get mood -o TEST-11)" = 2 ] || fail "imported values: $(ctl prop ls -o TEST-11)"
if sed 's/knobs/other/' "$scene/leaf.json" | ctl prop preset import - -o TEST-11 2>/dev/null; then fail "values of another package were imported"; fi
ctl prop share -o TEST-11
{ grep -q '^\[default.scenes."org.example.knobs"\]' "$config" && [ "$(settings "r['shared']")" = True ]; } ||
  fail "shared values: $(cat "$config")"
ctl prop set mood 1 -o TEST-11
{ [ "$(settings "r['shared'], r['values']['mood']")" = "(True, 1)" ] &&
  ! grep -q '^\[outputs.TEST-11.scenes."org.example.knobs"\]' "$config"; } || fail "an edit left the shared values: $(cat "$config")"
ctl prop share --own -o TEST-11
[ "$(settings "r['shared']")" = False ] || fail "own values"
grep -q hunter2 "$config" "$WORK"/*.log && fail "a secret reached the configuration or a log"
ctl lib apply "$reel" -o TEST-11
shown "$s11" 0 0 0
expect "s['outputs']['TEST-11']['accent']" "#123456"
steady() { for _ in $(seq 8); do looks "$@" || return 1; sleep 0.2; done; }
steady "$s11" 0 0 0 || fail "the video left its part: $("$BIN/probe" "$s11" 3)"
[ "$(settings "r['kind'], r['values']['speed'], r['values']['zoom'], r['values']['volume']")" = "('video', 2.0, 2.0, 50)" ] ||
  fail "video settings: $(ctl prop ls -o TEST-11)"
ctl prop set y -0.5 -o TEST-11 && shown "$s11" 0 255 255
ctl prop set x -0.5 -o TEST-11 && shown "$s11" 255 255 0
ctl prop set start 0 -o TEST-11 && shown "$s11" 255 0 0
ctl prop preset load slow -o TEST-11
[ "$(ctl prop get speed -o TEST-11)" = 0.5 ] && expect "s['outputs']['TEST-11']['kind']" video
shown "$s11" 255 0 0
ctl release TEST-11 && expect "list(s['outputs'])" "[]"
bus Library1 Remove asb 4 "$glow" "$clip" "$knobs" "$reel" false >/dev/null
echo "scene checks passed"
