#!/usr/bin/env bash
set -euo pipefail
full=$1 still=$2 green=$3 blue=$4
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
ctl lib apply "$scene" -o screen
check "$full" "0.25,0.5,$blue" "0.75,0.5,#000000"
region() { grep "pointer input of the background" "$WORK/wprsd.log" | tail -n 1 | grep -q "$1$"; }
taking() { ctl input status --json | python3 -c "import json, sys; sys.exit('screen' not in json.load(sys.stdin)['clicking'])"; }
ctl input grant org.example.clicks pointer
ctl input grant org.example.clicks clicks
wait_for 10 region true || fail "the wallpaper window did not take the pointer for the scene"
xdotool mousemove 320 360 click 1 && sleep 0.5
if taking; then fail "a screen took clicks before they were turned on"; fi
check "$full" "0.75,0.5,#000000"
ctl input clicks on -o screen
wait_for 10 taking || fail "the screen did not take clicks: $(ctl input status)"
xdotool click --window "$(xdotool search --class '^wprsd$' | head -n 1)" 1 && sleep 0.5
check "$full" "0.75,0.5,#000000"
xdotool mousemove 960 360 click 1 && sleep 0.5
check "$full" "0.75,0.5,#000000"
xdotool mousemove 320 360 click 1
check "$full" "0.75,0.5,#ffffff" "0.25,0.5,$blue"
ctl input revoke org.example.clicks pointer
ctl input revoke org.example.clicks clicks
wait_for 10 region false || fail "the wallpaper window kept the pointer after the inputs were revoked"
ctl input clicks off
ctl set "$still" --fit fit
check "$full" "0.5,0.1,$green" "0.02,0.02,$blue"
echo "real and synthesized click checks passed"

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
