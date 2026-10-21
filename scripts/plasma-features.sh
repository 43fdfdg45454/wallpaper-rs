#!/usr/bin/env bash
set -euo pipefail

WORK=$1
OUT=$2
ctl() { "$HOME/.local/bin/wprs-ctl" "$@"; }
field() { ctl state | jq -r "$1"; }
fail() { echo "FAIL: $*"; exit 1; }
output=$(field '.outputs | keys[0]')
at() { field ".outputs[\"$output\"].$1"; }
is() { [ "$(at "$1")" = "$2" ]; }

echo "===== video wallpaper on $output"
ctl set "$WORK/magenta.webm" --fit stretch
sleep 5
if ! is kind video || ! is playing true; then fail "the video is not playing: $(ctl state)"; fi
spectacle --background --nonotify --fullscreen --output "$OUT/video.png" >"$OUT/spectacle-video.log" 2>&1 || true
wait_for 20 test -s "$OUT/video.png" || fail "no screenshot of the video"
read -r w h < <(magick identify -format "%w %h\n" "$OUT/video.png")
rgb=$(magick "$OUT/video.png" -format "%[fx:int(255*p{$((w / 2)),$((h / 2))}.r)] %[fx:int(255*p{$((w / 2)),$((h / 2))}.g)] %[fx:int(255*p{$((w / 2)),$((h / 2))}.b)]" info:)
echo "video pixel: $rgb"
read -r r g b <<<"$rgb"
if [ "$r" -lt 160 ] || [ "$g" -gt 80 ] || [ "$b" -lt 50 ] || [ "$b" -gt 150 ]; then fail "the screenshot does not show the video"; fi

echo "===== a maximized window pauses the video"
printf 'import QtQuick\nWindow { visible: true; visibility: Window.Maximized; color: "white" }\n' >"$WORK/cover.qml"
/usr/lib/qt6/bin/qml "$WORK/cover.qml" >"$OUT/cover.log" 2>&1 &
cover=$!
wait_for 20 is covered true || fail "the plugin did not report the maximized window"
is playing false || fail "the video kept playing behind a maximized window"
kill "$cover"
wait_for 20 is covered false || fail "the plugin did not report the window closing"
wait_for 10 is playing true || fail "the video did not resume"

echo "===== tray icon"
calls=$(grep -c "member=GetState" "$OUT/dbus-monitor.log" || true)
timeout 15 plasmawindowed io.github.wallpaper_rs.tray >"$OUT/tray.log" 2>&1 || true
grep -E "io.github.wallpaper_rs.tray.*(Error|error|not a type|not installed|Cannot)" "$OUT/tray.log" && fail "the tray icon has QML errors"
[ "$(grep -c "member=GetState" "$OUT/dbus-monitor.log" || true)" -gt "$calls" ] || fail "the tray icon never asked for the state"

echo "plasma test passed"
