#!/usr/bin/env bash
set -euo pipefail

BIN=$(realpath "${1:?usage: $0 <bin-dir>}")
ROOT=$(cd "$(dirname "$0")/.." && pwd)
WORK=$(mktemp -d /tmp/wprs-sway.XXXXXX)
export XDG_RUNTIME_DIR=$WORK/run XDG_CONFIG_HOME=$WORK/config XDG_DATA_HOME=$WORK/data XDG_CACHE_HOME=$WORK/cache XDG_STATE_HOME=$WORK/state
export WLR_BACKENDS=headless WLR_RENDERER=pixman WLR_HEADLESS_OUTPUTS=2 WLR_LIBINPUT_NO_DEVICES=1
unset WAYLAND_DISPLAY DISPLAY SWAYSOCK HYPRLAND_INSTANCE_SIGNATURE
mkdir -p "$XDG_RUNTIME_DIR" "$XDG_CONFIG_HOME"
chmod 700 "$XDG_RUNTIME_DIR"

pids=()
cleanup() {
  for p in "${pids[@]}"; do kill "$p" 2>/dev/null || true; done
  if [ -n "${DBUS_SESSION_BUS_PID:-}" ]; then kill "$DBUS_SESSION_BUS_PID" 2>/dev/null || true; fi
  if [ "${status:-1}" != 0 ]; then
    for f in "$WORK"/*.log; do echo "== $f"; tail -n 40 "$f"; done
  fi
  rm -rf "$WORK"
}
trap cleanup EXIT

wait_for() {
  local seconds=$1; shift
  for _ in $(seq $((seconds * 10))); do "$@" >/dev/null 2>&1 && return 0; sleep 0.1; done
  return 1
}

ctl() { "$BIN/wprs-ctl" "$@"; }
field() { ctl state | python3 -c "import json, sys; s = json.load(sys.stdin); print(eval(sys.argv[1]))" "$1"; }
expect() {
  for _ in $(seq 50); do [ "$(field "$1")" = "$2" ] && return; sleep 0.1; done
  echo "FAIL: $1 is $(field "$1"), expected $2" && exit 1
}
fail() { echo "FAIL: $*"; exit 1; }

shows() {
  timeout 5 grim -o "$1" -t ppm "$WORK/shot.ppm" && python3 - "$WORK/shot.ppm" "${@:2}" <<'EOF'
import re, sys
data = open(sys.argv[1], "rb").read()
header = re.match(rb"P6\s+(\d+)\s+(\d+)\s+255\s", data)
width, height = int(header[1]), int(header[2])
for spot in sys.argv[2:]:
    fx, fy, color = spot.split(",")
    x, y = int(float(fx) * (width - 1)), int(float(fy) * (height - 1))
    i = header.end() + (y * width + x) * 3
    want = bytes.fromhex(color[1:])
    if any(abs(a - b) > 24 for a, b in zip(data[i:i + 3], want)):
        sys.exit(f"{spot}: got #{data[i:i + 3].hex()} in {width}x{height}")
EOF
}
check() { wait_for 10 shows "$@" || { shows "$@"; fail "$1 does not show ${*:2}"; }; }

eval "$(dbus-launch --sh-syntax)"
pipewire >"$WORK/pipewire.log" 2>&1 & pipewire=$!; pids+=("$pipewire")
wait_for 10 test -S "$XDG_RUNTIME_DIR/pipewire-0"
wireplumber >"$WORK/wireplumber.log" 2>&1 & wireplumber=$!; pids+=("$wireplumber")

cat >"$WORK/sway.conf" <<'EOF'
xwayland disable
output HEADLESS-1 mode --custom 640x360@60Hz pos 0 0
output HEADLESS-2 mode --custom 640x360@60Hz scale 1.5 pos 640 0
output HEADLESS-3 mode --custom 480x270@120Hz
for_window [app_id="foot"] fullscreen enable
EOF
sway -c "$WORK/sway.conf" >"$WORK/sway.log" 2>&1 & sway=$!; pids+=("$sway")
wait_for 10 compgen -G "$XDG_RUNTIME_DIR/sway-ipc.*.sock" || fail "sway did not start"
SWAYSOCK=$(compgen -G "$XDG_RUNTIME_DIR/sway-ipc.*.sock")
WAYLAND_DISPLAY=$(basename "$(compgen -G "$XDG_RUNTIME_DIR/wayland-[0-9]")")
export SWAYSOCK WAYLAND_DISPLAY

log=${WPRS_IT_LOG:-debug},naga=warn,wgpu_core=warn,wgpu_hal=warn
RUST_LOG=$log "$BIN/wprsd" --backend layer-shell >"$WORK/wprsd.log" 2>&1 & wprsd=$!; pids+=("$wprsd")
wait_for 30 ctl state || fail "the daemon did not start"
expect "sorted(s['outputs'])" "['HEADLESS-1', 'HEADLESS-2']"
expect "[s['outputs'][o]['height'] for o in ('HEADLESS-1', 'HEADLESS-2')]" "[360, 360]"
grep -q "presentation backend chosen.*layer-shell" "$WORK/wprsd.log" || fail "layer-shell not chosen"

green='#00ff00' blue='#2040c0' magenta='#d02060'
ctl background "$blue"
ctl set "$ROOT/tests/fixtures/green-64x32.png" --fit fit
expect "s['outputs']['HEADLESS-1']['kind']" image
for output in HEADLESS-1 HEADLESS-2; do
  check "$output" "0.5,0.5,$green" "0.02,0.02,$blue" "0.98,0.98,$blue" "0.02,0.5,$green"
done
echo "image and background checks passed"

cp "$ROOT/tests/fixtures/bands-640x180.png" "$WORK/bands.png"
ctl set "$WORK/bands.png" --fit span
check HEADLESS-1 "0.1,0.5,#ff0000" "0.9,0.5,$green"
check HEADLESS-2 "0.1,0.5,$green" "0.9,0.5,#0000ff"
expect "s['outputs']['HEADLESS-2']['screen']['scale']" 1.5
ctl edit set "$WORK/bands.png" ev=-1
expect "s['outputs']['HEADLESS-1']['edited']" True
echo "span checks passed"

ticks() { awk '{print $14 + $15}' "/proc/$wprsd/stat"; }
settled() { local start; start=$(ticks); sleep 0.5; [ $(($(ticks) - start)) -le 2 ]; }
wait_for 10 settled || fail "the daemon did not settle after the edit"
before=$(ticks); sleep 2
[ $(($(ticks) - before)) -le 20 ] || fail "the daemon kept working with a still wallpaper"
echo "idle check passed"

ctl set "$ROOT/tests/fixtures/green-64x32.png" --fit fit
if ctl stream HEADLESS-1 160 90 2>"$WORK/stream.err"; then fail "a stream took the name of a screen"; fi
grep -q "on its own surface" "$WORK/stream.err" || fail "stream named like a screen: $(cat "$WORK/stream.err")"

ctl stream HEADLESS-3 160 90 >/dev/null
swaymsg -q create_output
expect "sorted(s['outputs'])" "['HEADLESS-1', 'HEADLESS-2', 'HEADLESS-3']"
wait_for 10 grep -q "closing a stream named after a screen" "$WORK/wprsd.log" || fail "the stream of a new screen was kept"
expect "[s['outputs']['HEADLESS-3'][k] for k in ('width', 'height', 'clients')]" "[480, 270, 1]"
expect "[s['outputs'][o]['refresh'] for o in ('HEADLESS-1', 'HEADLESS-3')]" "[60, 120]"
grep -q "presented without a refresh rate" "$WORK/wprsd.log" || fail "no presentation feedback"
check HEADLESS-3 "0.5,0.5,$green" "0.02,0.02,$blue"
swaymsg -q output HEADLESS-3 unplug
expect "sorted(s['outputs'])" "['HEADLESS-1', 'HEADLESS-2']"
surfaces() { grep "surface ready" "$WORK/wprsd.log" | grep -c "$1"; }
before=$(surfaces HEADLESS-2)
swaymsg -q output HEADLESS-2 disable && swaymsg -q output HEADLESS-2 enable
check HEADLESS-2 "0.5,0.5,$green" "0.02,0.02,$blue"
sleep 1
if grep "output removed" "$WORK/wprsd.log" | grep -q HEADLESS-2; then fail "a short output flap removed the output"; fi
[ $(($(surfaces HEADLESS-2) - before)) -le 1 ] || fail "a short output flap rebuilt the surface more than once"
echo "hotplug checks passed"

ctl set "$ROOT/tests/fixtures/magenta-64x32.webm" --fit stretch
expect "s['outputs']['HEADLESS-1']['playing']" True
expect "s['outputs']['HEADLESS-2']['playing']" True
check HEADLESS-1 "0.5,0.5,$magenta"
swaymsg -q focus output HEADLESS-1
foot >"$WORK/foot.log" 2>&1 & foot=$!; pids+=("$foot")
expect "s['outputs']['HEADLESS-1']['covered']" True
expect "s['outputs']['HEADLESS-1']['playing']" False
expect "s['outputs']['HEADLESS-2']['playing']" True
swaymsg -q output HEADLESS-1 disable && kill "$foot" && swaymsg -q output HEADLESS-1 enable
expect "s['outputs']['HEADLESS-1']['covered']" False
expect "s['outputs']['HEADLESS-1']['playing']" True
echo "fullscreen pause checks passed"

swaymsg -q output HEADLESS-2 power off
expect "s['outputs']['HEADLESS-2']['covered']" True
expect "s['outputs']['HEADLESS-2']['playing']" False
swaymsg -q output HEADLESS-2 power on
expect "s['outputs']['HEADLESS-2']['playing']" True
echo "frame callback starvation checks passed"

before=$(surfaces HEADLESS-2)
rebuilt() { [ "$(surfaces HEADLESS-2)" -gt "$before" ]; }
swaymsg -q output HEADLESS-2 disable && swaymsg -q output HEADLESS-2 enable
wait_for 10 rebuilt || fail "a short output flap did not rebuild the surface"
sleep 1
expect "s['outputs']['HEADLESS-2']['covered']" False
expect "s['outputs']['HEADLESS-2']['playing']" True
echo "video output flap checks passed"

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

status=0
echo "sway test passed"
