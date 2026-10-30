#!/usr/bin/env bash
set -euo pipefail
declare blue log sway wireplumber
mkdir -p "$WORK/clicks"
cat >"$WORK/clicks/scene.json" <<'SCENE'
{"schema": "wallpaper-rs.scene/1", "canvas": {"size": [64, 36], "background": "#000000"},
 "layers": [{"id": "button", "type": "solid", "color": "#2040c0", "size": [32, 36], "solid": true, "transform": {"position": [16, 18]}},
  {"id": "lamp", "type": "solid", "color": "#ffffff", "size": [32, 36], "transform": {"position": [48, 18]},
   "opacity": {"timeline": "light", "keys": [[0, 0], [0.1, 1]]}}],
 "timelines": [{"id": "light", "duration": 0.1, "autoplay": false, "trigger": "click:button"}]}
SCENE
cat >"$WORK/clicks/manifest.json" <<'MANIFEST'
{"schema": "wallpaper-rs.wpak/2", "id": "org.example.clicks", "version": "1.0.0", "title": "Clicks", "license": "CC0-1.0",
 "kind": "scene", "entry": "scene.json", "content": {"rating": "general"}, "photosensitive": false,
 "origin": {"kind": "original"}, "inputs": ["pointer", "clicks"]}
MANIFEST
ctl pack build "$WORK/clicks" -o "$WORK/clicks.wpak" >/dev/null
ctl lib import "$WORK/clicks.wpak" --wait >/dev/null
scene=$(ctl lib search kind:scene --json | python3 -c "import json, sys; print(json.load(sys.stdin)['items'][0]['uid'])")
ctl lib apply "$scene" -o HEADLESS-1
check HEADLESS-1 "0.25,0.5,$blue" "0.75,0.5,#000000"
mkfifo "$WORK/pointer.in"
"$BIN/examples/pointer" <"$WORK/pointer.in" >"$WORK/pointer.log" 2>&1 & pids+=($!)
exec 7>"$WORK/pointer.in"
wait_for 10 grep -q ready "$WORK/pointer.log" || fail "no virtual pointer"
click() { printf '%s %s 1067 360\n1 1\n1 0\n' "$1" "$2" >&7; sleep 0.5; }
region() { grep "pointer input of the background" "$WORK/wprsd.log" | grep "HEADLESS-1" | tail -n 1 | grep -q "$1$"; }
taking() { ctl input status --json | python3 -c "import json, sys; sys.exit('HEADLESS-1' not in json.load(sys.stdin)['clicking'])"; }
ctl input grant org.example.clicks pointer -o HEADLESS-1
ctl input grant org.example.clicks clicks -o HEADLESS-1
wait_for 10 region true || fail "the background did not take the pointer for the scene"
click 160 180
if taking; then fail "a screen took clicks before they were turned on"; fi
check HEADLESS-1 "0.75,0.5,#000000"
ctl input clicks on -o HEADLESS-1
wait_for 10 taking || fail "the screen did not take clicks: $(ctl input status)"
click 480 180
check HEADLESS-1 "0.75,0.5,#000000"
click 160 180
check HEADLESS-1 "0.75,0.5,#ffffff" "0.25,0.5,$blue"
cursor() {
  timeout 5 grim -c -o HEADLESS-1 -t ppm "$WORK/cursor.ppm" && python3 - "$WORK/cursor.ppm" <<'CURSOR'
import re, sys
data = open(sys.argv[1], "rb").read()
header = re.match(rb"P6\s+(\d+)\s+(\d+)\s+255\s", data)
width = int(header[1])
spots = [data[header.end() + ((180 + dy) * width + 160 + dx) * 3:][:3] for dx in range(1, 6) for dy in range(2, 12)]
sys.exit(all(abs(s[0] - 0x20) < 24 and abs(s[2] - 0xc0) < 24 for s in spots))
CURSOR
}
wait_for 5 cursor || fail "no cursor over the background that takes the pointer"
ctl input revoke org.example.clicks pointer
ctl input revoke org.example.clicks clicks
wait_for 10 region false || fail "the background kept the pointer after the inputs were revoked"
ctl input clicks off
echo "real click, input region and cursor checks passed"

mkdir -p "$WORK/hear"
echo '{"schema": "wallpaper-rs.wpak/2", "id": "org.example.hear", "version": "1.0.0", "title": "Hear", "license": "CC0-1.0",
 "kind": "scene", "entry": "scene.json", "content": {"rating": "general"}, "photosensitive": false,
 "origin": {"kind": "original"}, "inputs": ["audio"]}' >"$WORK/hear/manifest.json"
echo '{"schema": "wallpaper-rs.scene/1", "canvas": {"size": [64, 36]}, "layers": [{"id": "bars", "type": "spectrum", "size": [64, 36]}]}' \
  >"$WORK/hear/scene.json"
ctl pack build "$WORK/hear" -o "$WORK/hear.wpak" >/dev/null && ctl lib import "$WORK/hear.wpak" --wait >/dev/null
speaker() {
  pw-cli create-node adapter '{ factory.name=support.null-audio-sink node.name=wprs-sway-sink media.class=Audio/Sink
    object.linger=true audio.position=[FL FR] }' >/dev/null
}
speaker
ctl lib apply "$(ctl lib search kind:scene --json | python3 -c "import json, sys; print([i['uid'] for i in json.load(sys.stdin)['items'] if i['title'] == 'Hear'][0])")" -o HEADLESS-2
ctl input grant org.example.hear audio -o HEADLESS-2
capturing() { [ "$(ctl input status --json | python3 -c "import json, sys; print(json.load(sys.stdin)['capturing'])")" = "$1" ]; }
analysing() {
  local dump
  dump=$(pw-dump 2>/dev/null) || return 1
  grep -q '"media.name": "wallpaper-rs audio analysis"' <<<"$dump"
}
heard() { capturing True && analysing; }
silent() { capturing False && ! analysing; }
wait_for 10 heard || fail "no audio capture for the scene that asks for it: $(ctl input status)"
ctl config playback.scene_capture false
wait_for 10 silent || fail "the capture went on after it was turned off for every scene"
ctl config playback.scene_capture true
wait_for 10 heard || fail "the capture did not come back when it was allowed again"
kill "$wireplumber" "$pipewire"
wait "$wireplumber" "$pipewire" 2>/dev/null || true
pipewire >"$WORK/pipewire-again.log" 2>&1 & pipewire=$!; pids+=("$pipewire")
wireplumber >"$WORK/wireplumber-again.log" 2>&1 & pids+=($!)
wait_for 10 speaker || fail "no audio output after PipeWire restarted"
wait_for 30 analysing || fail "the capture did not reconnect after PipeWire restarted"
capturing True || fail "the capture stopped when PipeWire restarted"
grep -q "the audio capture is connected to PipeWire again" "$WORK/wprsd.log" || fail "the capture was opened again instead of reconnecting"
echo "audio capture switch and PipeWire restart checks passed"

kill "$wprsd"
wait "$wprsd" || true
RUST_LOG=$log "$BIN/wprsd" --backend streams >"$WORK/streams.log" 2>&1 & wprsd=$!; pids+=("$wprsd")
wait_for 30 ctl state || fail "the daemon did not start with streams"
swaymsg -q "output HEADLESS-1 mode --custom 640x360@120Hz"
ctl stream HEADLESS-1 1280 720 >/dev/null && ctl stream HEADLESS-2 1920 1080 >/dev/null
ctl stream OTHER-1 1280 720 >/dev/null
expect "[(s['outputs'][o]['width'], s['outputs'][o]['height']) for o in ('HEADLESS-1', 'HEADLESS-2', 'OTHER-1')]" \
  "[(640, 360), (640, 360), (1280, 720)]"
expect "[s['outputs'][o]['refresh'] for o in ('HEADLESS-1', 'HEADLESS-2', 'OTHER-1')]" "[120, 60, 0]"
echo "stream size and refresh rate of the output mode checks passed"

kill "$sway"; wait "$sway" 2>/dev/null || true
sed -i 's/HEADLESS-1 mode --custom 640x360/HEADLESS-1 mode --custom 960x540/' "$WORK/sway.conf"
sway -c "$WORK/sway.conf" >"$WORK/sway-restarted.log" 2>&1 & pids+=($!)
wait_for 10 test -S "$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY" || fail "sway did not come back on $WAYLAND_DISPLAY"
followed() { [ "$(field "(s['outputs']['HEADLESS-1']['width'], s['outputs']['HEADLESS-1']['height'])")" = "(960, 540)" ]; }
wait_for 30 followed || fail "the stream did not follow the mode of the restarted compositor"
echo "compositor restart checks passed"
