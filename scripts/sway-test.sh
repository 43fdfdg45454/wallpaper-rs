#!/usr/bin/env bash
set -euo pipefail

BIN=$(realpath "${1:?usage: $0 <bin-dir>}")
ROOT=$(cd "$(dirname "$0")/.." && pwd)
WORK=$(mktemp -d /tmp/wprs-sway.XXXXXX)
export XDG_RUNTIME_DIR=$WORK/run XDG_CONFIG_HOME=$WORK/config
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
pipewire >"$WORK/pipewire.log" 2>&1 & pids+=($!)
wait_for 10 test -S "$XDG_RUNTIME_DIR/pipewire-0"
wireplumber >"$WORK/wireplumber.log" 2>&1 & pids+=($!)

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
