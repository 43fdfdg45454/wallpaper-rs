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

echo "===== the tray finds the running app and opens its settings page"
ui=$HOME/.local/share/plasma/plasmoids/io.github.wallpaper_rs.tray/contents/ui
[ -f "$ui/Daemon.qml" ] || fail "the tray is not installed in $ui"
probe() {
  printf 'import QtQuick\nimport "file://%s"\nDaemon {\n    hasApp: %s\n    onHasAppChanged: %s\n    Component.onCompleted: findApp()\n}\n' \
    "$ui" "$2" "$3" >"$WORK/$1.qml"
}
qml() { QT_QPA_PLATFORM=offscreen timeout 60 /usr/lib/qt6/bin/qml "$WORK/$1.qml" >"$OUT/tray-$1.log" 2>&1; }
trap 'kill "${app:-}" "${monitor:-}" "${private:-}" 2>/dev/null || true' EXIT
dbus-monitor --session "interface='io.github.wallpaper_rs.Library1',member='ListSources'" >"$OUT/app-monitor.log" 2>&1 &
monitor=$!
count() { grep -c "member=ListSources" "$OUT/app-monitor.log" || true; }
listed() { [ "$(count)" -gt "$1" ]; }
gone() { ! bus_has io.github.wallpaper_rs; }
activatable() {
  dbus-send --session --print-reply --dest=org.freedesktop.DBus /org/freedesktop/DBus \
    org.freedesktop.DBus.ListActivatableNames | grep -q "\"$1\""
}
"$HOME/.local/bin/wprs-app" --page library >"$OUT/app.log" 2>&1 &
app=$!
wait_for 60 bus_has io.github.wallpaper_rs || fail "wprs-app did not start"
probe open false 'openApp("settings")'
qml open &
wait_for 30 listed 0 || fail "the Settings button of the tray did not open the settings page of the running app"
pkill -f "$WORK/open.qml" || true
kill "$app"
wait_for 20 gone || fail "wprs-app did not quit"

echo "===== the launcher action opens the app on its settings page"
before=$(count)
"$HOME/.local/bin/wprs-app" --page settings >"$OUT/app-page.log" 2>&1 &
app=$!
wait_for 60 listed "$before" || fail "wprs-app --page settings did not open the settings page"
kill "$app" "$monitor"

echo "===== the tray offers the app only where it is installed"
services=$WORK/services
mkdir -p "$services"
cp "$HOME/.local/share/dbus-1/services/io.github.wallpaper_rs.Daemon.service" "$services/"
printf '<busconfig><listen>unix:dir=%s</listen><auth>EXTERNAL</auth><servicedir>%s</servicedir><policy context="default"><allow send_destination="*"/><allow receive_sender="*"/><allow own="*"/></policy></busconfig>\n' \
  "$WORK" "$services" >"$WORK/bus.conf"
dbus-daemon --nofork --print-address --config-file="$WORK/bus.conf" >"$WORK/bus.address" &
private=$!
wait_for 10 test -s "$WORK/bus.address" || fail "the private bus did not start"
session=$DBUS_SESSION_BUS_ADDRESS
DBUS_SESSION_BUS_ADDRESS=$(head -n 1 "$WORK/bus.address")
probe flatpak true 'Qt.quit()'
qml flatpak || fail "the tray offered the app where only the daemon is installed"
cp "$HOME/.local/share/dbus-1/services/io.github.wallpaper_rs.service" "$services/"
wait_for 10 activatable io.github.wallpaper_rs || fail "the private bus did not list the app"
probe installed false 'Qt.quit()'
qml installed || fail "the tray did not find the installed app"
DBUS_SESSION_BUS_ADDRESS=$session

echo "plasma test passed"
