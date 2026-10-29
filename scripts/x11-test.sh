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
lower() {
  python3 - "${DISPLAY#:}" "$1" "$2" <<'LOWER'
import socket, struct, sys, time
x = socket.socket(socket.AF_UNIX)
x.connect(f"/tmp/.X11-unix/X{sys.argv[1]}")
x.sendall(struct.pack("<cxHHHHxx", b"l", 11, 0, 0, 0))
head = x.recv(8)
left = struct.unpack("<H", head[6:8])[0] * 4
while left > 0:
    left -= len(x.recv(left))
for _ in range(int(sys.argv[3])):
    x.sendall(struct.pack("<BxHIHxxI", 12, 4, int(sys.argv[2]), 0x40, 1))
    time.sleep(0.05)
x.sendall(struct.pack("<BxH", 43, 1))
x.recv(32)
LOWER
}
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
expect "[s['outputs']['screen'][k] for k in ('width', 'height', 'refresh')]" "[1280, 720, 0]"
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
expect "s['outputs']['DP-1']['covered']" True
xdotool windowsize "$window" 320 360
expect "s['outputs']['DP-1']['covered']" False
expect "s['outputs']['DP-1']['playing']" True
xprop -root -remove _NET_CLIENT_LIST_STACKING
xdotool windowsize "$window" 640 720
expect "s['outputs']['DP-1']['covered']" True
kill "$xev"
expect "s['outputs']['DP-1']['covered']" False
echo "fullscreen and hidden window pause checks passed"

xev -geometry 200x200+700+100 -name bottom >"$WORK/bottom.log" 2>&1 & bottom=$!; pids+=("$bottom")
low=$(timeout 10 xdotool search --sync --name '^bottom$' | head -n 1)
lower "$low" 30
wait_for 10 grep -q "keeps its window at the bottom" "$WORK/wprsd.log" || fail "the daemon fought another window for the bottom"
[ "$(grep -c "keeps its window at the bottom" "$WORK/wprsd.log")" = 1 ] || fail "the daemon kept fighting for the bottom"
kill "$bottom"
echo "bottom window checks passed"

ctl pause && expect "s['outputs']['DP-2']['playing']" False
opened=$(grep -c "video opened" "$WORK/wprsd.log")
xev -geometry 300x200+800+260 >"$WORK/xev.log" 2>&1 & xev=$!; pids+=("$xev")
timeout 10 xdotool search --sync --onlyvisible --name '^Event Tester$' >/dev/null
kill "$xev"
wait "$xev" 2>/dev/null || true
check 640x720+640+0 "0.5,0.5,$magenta"
expect "s['outputs']['DP-2']['playing']" False
[ "$(grep -c "video opened" "$WORK/wprsd.log")" = "$opened" ] || fail "an exposure restarted the paused video"
ctl resume
echo "exposure of a paused video checks passed"

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

desks() {
  local w found=0
  for w in $(xdotool search --class '^wprsd$'); do
    xwininfo -id "$w" | grep -q "Override Redirect State: $1" || return 1
    found=1
  done
  [ "$found" = 1 ]
}
wait_for 10 desks yes || fail "the wallpaper window is not override-redirect without a window manager"
openbox --sm-disable >"$WORK/openbox.log" 2>&1 & wm=$!; pids+=("$wm")
wait_for 30 desks no || fail "the window manager did not get the wallpaper window"
check "$full" "0.5,0.1,$green" "0.02,0.02,$blue"
xev -geometry 1280x720+0+0 -name icons >"$WORK/icons.log" 2>&1 & icons=$!; pids+=("$icons")
desktop=$(timeout 10 xdotool search --sync --name '^icons$' | head -n 1)
xdotool windowunmap --sync "$desktop"
xprop -id "$desktop" -f _NET_WM_WINDOW_TYPE 32a -set _NET_WM_WINDOW_TYPE _NET_WM_WINDOW_TYPE_DESKTOP
xdotool windowmap "$desktop"
below() {
  local ours
  mapfile -t ours < <(xdotool search --class '^wprsd$')
  xprop -root _NET_CLIENT_LIST_STACKING | python3 -c "import sys
order = [int(w, 16) for w in sys.stdin.read().split('#')[1].replace(',', ' ').split()]
ours = [int(w) for w in sys.argv[2:]]
sys.exit(not all(order.index(o) < order.index(int(sys.argv[1])) for o in ours))" "$desktop" "${ours[@]}"
}
wait_for 10 below || fail "the wallpaper stayed above the desktop icons: $(xprop -root _NET_CLIENT_LIST_STACKING)"
xdotool search --class '^wprsd$' windowraise %@ && sleep 1
wait_for 10 below || fail "a raised wallpaper stayed above the desktop icons: $(xprop -root _NET_CLIENT_LIST_STACKING)"
expect "s['outputs']['screen']['covered']" True
kill "$icons"
expect "s['outputs']['screen']['covered']" False
echo "desktop icon window checks passed"
kill "$wm"
wait "$wm" 2>/dev/null || true
wait_for 30 desks yes || fail "the wallpaper window was not taken back from the window manager"
check "$full" "0.5,0.1,$green" "0.02,0.02,$blue"
echo "window manager checks passed"

xdotool search --class '^wprsd$' windowunmap %@
check "$full" "0.5,0.1,$green" "0.02,0.02,$blue" "0.98,0.98,$blue"
kill "$wprsd"
wait "$wprsd" || true
check "$full" "0.5,0.1,$green" "0.02,0.02,$blue" "0.98,0.98,$blue"
echo "root background checks passed"

status=0
echo "X11 test passed"
