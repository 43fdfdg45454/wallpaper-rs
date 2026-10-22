#!/usr/bin/env bash
set -euo pipefail

BIN=$(realpath "${1:?usage: $0 <bin-dir>}")
ROOT=$(cd "$(dirname "$0")/.." && pwd)
WORK=$(mktemp -d /tmp/wprs-x11.XXXXXX)
export XDG_RUNTIME_DIR=$WORK/run XDG_CONFIG_HOME=$WORK/config XDG_DATA_HOME=$WORK/data XDG_CACHE_HOME=$WORK/cache
unset WAYLAND_DISPLAY DISPLAY XDG_SESSION_TYPE XDG_CURRENT_DESKTOP
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
  import -silent -window root -crop "$1" +repage -depth 8 "$WORK/shot.ppm" && python3 - "$WORK/shot.ppm" "${@:2}" <<'EOF'
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
pixmap() { xprop -root "$1" | awk '/# 0x/ {print $NF}'; }
rooted() { [ -n "$(pixmap _XROOTPMAP_ID)" ]; }

eval "$(dbus-launch --sh-syntax)"
pipewire >"$WORK/pipewire.log" 2>&1 & pids+=($!)
wait_for 10 test -S "$XDG_RUNTIME_DIR/pipewire-0"
wireplumber >"$WORK/wireplumber.log" 2>&1 & pids+=($!)

Xvfb -displayfd 3 -screen 0 1280x720x24 -nolisten tcp -noreset 3>"$WORK/display" >"$WORK/xvfb.log" 2>&1 &
pids+=($!)
wait_for 60 test -s "$WORK/display" || fail "Xvfb did not start"
DISPLAY=:$(cat "$WORK/display")
export DISPLAY

log=${WPRS_IT_LOG:-debug},naga=warn,wgpu_core=warn,wgpu_hal=warn
RUST_LOG=$log "$BIN/wprsd" --backend x11 >"$WORK/wprsd.log" 2>&1 & wprsd=$!; pids+=("$wprsd")
wait_for 30 ctl state || fail "the daemon did not start"
expect "sorted(s['outputs'])" "['screen']"
expect "[s['outputs']['screen'][k] for k in ('width', 'height')]" "[1280, 720]"
grep -q "presentation backend chosen.*x11" "$WORK/wprsd.log" || fail "X11 not chosen"

green='#00ff00' blue='#2040c0' magenta='#d02060' full=1280x720+0+0
ctl background "$blue"
check "$full" "0.5,0.5,$blue" "0.02,0.98,$blue"
ctl set "$ROOT/tests/fixtures/green-64x32.png" --fit fit
expect "s['outputs']['screen']['kind']" image
check "$full" "0.5,0.5,$green" "0.02,0.02,$blue" "0.98,0.98,$blue"
wait_for 10 rooted || fail "no root pixmap"
[ "$(pixmap _XROOTPMAP_ID)" = "$(pixmap ESETROOT_PMAP_ID)" ] || fail "the root pixmap properties differ"
exposed() {
  xev -geometry 400x300+300+200 >"$WORK/xev.log" 2>&1 & local xev=$!
  timeout 10 xdotool search --sync --onlyvisible --name '^Event Tester$'
  kill "$xev" && wait "$xev"
  shows 400x300+300+200 "0.5,0.5,$green" "0.98,0.02,$green"
}
wait_for 3 exposed || fail "a window left its trace on the wallpaper"
echo "color, image, root pixmap and exposure checks passed"

xrandr --setmonitor DP-1 640/169x720/190+0+0 screen >/dev/null
xrandr --setmonitor DP-2 640/169x720/190+640+0 none
expect "sorted(s['outputs'])" "['DP-1', 'DP-2']"
for area in 640x720+0+0 640x720+640+0; do
  check "$area" "0.5,0.5,$green" "0.5,0.1,$blue" "0.5,0.9,$blue" "0.02,0.5,$green"
done
echo "second monitor checks passed"

ticks() { awk '{print $14 + $15}' "/proc/$wprsd/stat"; }
before=$(ticks); sleep 2
[ $(($(ticks) - before)) -le 20 ] || fail "the daemon kept working with a still wallpaper"
echo "idle check passed"

ctl set "$ROOT/tests/fixtures/magenta-64x32.webm" --fit stretch
expect "s['outputs']['DP-1']['playing']" True
expect "s['outputs']['DP-2']['playing']" True
check 640x720+640+0 "0.5,0.5,$magenta"
xev -geometry 640x720+0+0 >"$WORK/xev.log" 2>&1 & xev=$!; pids+=("$xev")
window=$(timeout 10 xdotool search --sync --name '^Event Tester$' | head -n 1)
xprop -id "$window" -f _NET_WM_STATE 32a -set _NET_WM_STATE _NET_WM_STATE_FULLSCREEN
xprop -root -f _NET_CLIENT_LIST_STACKING 32x -set _NET_CLIENT_LIST_STACKING "$window"
expect "s['outputs']['DP-1']['covered']" True
expect "s['outputs']['DP-1']['playing']" False
expect "s['outputs']['DP-2']['playing']" True
xprop -id "$window" -remove _NET_WM_STATE
expect "s['outputs']['DP-1']['covered']" False
expect "s['outputs']['DP-1']['playing']" True
kill "$xev"
echo "fullscreen pause checks passed"

xrandr --delmonitor DP-2
xrandr --delmonitor DP-1
expect "sorted(s['outputs'])" "['screen']"
still=$WORK/still.png
cp "$ROOT/tests/fixtures/green-64x32.png" "$still"
case $(field "s['adapter']") in
  llvmpipe*) ctl edit depth import "$still" "$still" >/dev/null && ctl edit parallax "$still" --source auto ;;
esac
xprop -root -remove _XROOTPMAP_ID
ctl set "$still" --fit fit
check "$full" "0.5,0.1,$green" "0.02,0.02,$blue"
expect "s['outputs']['screen']['parallax']" False
wait_for 10 rooted || fail "no root pixmap for a still image"
echo "hotplug checks passed"

xdotool search --class '^wprsd$' windowunmap %@
check "$full" "0.5,0.1,$green" "0.02,0.02,$blue" "0.98,0.98,$blue"
kill "$wprsd"
wait "$wprsd" || true
check "$full" "0.5,0.1,$green" "0.02,0.02,$blue" "0.98,0.98,$blue"
echo "root background checks passed"

status=0
echo "X11 test passed"
