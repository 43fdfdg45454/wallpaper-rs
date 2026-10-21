#!/usr/bin/env bash
set -euo pipefail

OUT=$1

sleep 15
"$HOME/.local/bin/wprs-ctl" state | tee "$OUT/state.json"

echo "===== KPipeWire stream states"
grep -E "kpipewire_logging: (state changed|created successfully|Stream error)" "$OUT/plasmashell.log" || true
fail=0
if ! grep -q "kpipewire_logging: state changed paused -> streaming" "$OUT/plasmashell.log"; then
  echo "FAIL: the wallpaper stream never started streaming in plasmashell"
  fail=1
fi

if grep -q "received signal" "$OUT/plasmashell.log" || ! bus_has org.kde.plasmashell; then
  echo "FAIL: plasmashell crashed"
  grep -A 30 "received signal" "$OUT/plasmashell.log" | head -n 40 || true
  fail=1
fi

spectacle --background --nonotify --fullscreen --output "$OUT/screenshot.png" \
  >"$OUT/spectacle.log" 2>&1 || true
wait_for 20 test -s "$OUT/screenshot.png" || true
shot_ok=1
if [ ! -s "$OUT/screenshot.png" ]; then
  echo "no screenshot; spectacle said:"
  grep -v "kf.service" "$OUT/spectacle.log" | tail -n 20
  shot_ok=0
else
  pixel() {
    magick "$OUT/screenshot.png" \
      -format "%[fx:int(255*p{$1,$2}.r)],%[fx:int(255*p{$1,$2}.g)],%[fx:int(255*p{$1,$2}.b)]" info:
  }
  read -r shot_w shot_h < <(magick identify -format "%w %h\n" "$OUT/screenshot.png")
  echo "screenshot: ${shot_w}x${shot_h}"
  for xy in "$((shot_w / 2)) $((shot_h / 2))" "$((shot_w / 10)) $((shot_h / 7))" "$((shot_w * 9 / 10)) $((shot_h / 7))"; do
    read -r x y <<<"$xy"
    rgb=$(pixel "$x" "$y")
    echo "pixel ($xy): $rgb"
    IFS=, read -r r g b <<<"$rgb"
    if [ "$r" -gt 40 ] || [ "$g" -lt 200 ] || [ "$b" -gt 40 ]; then shot_ok=0; fi
  done
fi
if [ "$shot_ok" = 0 ]; then
  echo "FAIL: the screenshot does not show the wallpaper"
  fail=1
fi

if [ -s "$OUT/screenshot.png" ]; then
  echo "===== screenshot colors"
  magick "$OUT/screenshot.png" -format %c histogram:info:- | sort -rn | head -n 5
fi
echo "===== desktop config written by Plasma"
cat "$HOME/.config/plasma-org.kde.plasma.desktop-appletsrc" || true
echo "===== D-Bus calls to the daemon"
cat "$OUT/dbus-monitor.log" || true
echo "===== current activity"
dbus-send --session --print-reply --dest=org.kde.ActivityManager /ActivityManager/Activities \
  org.kde.ActivityManager.Activities.CurrentActivity 2>&1 | tail -n 1 || true
echo "===== plasmashell messages (screens, views, containments, plugin)"
grep -i -E "screen|view|corona|containment|activit|wallpaper|qml|pipewire|output" \
  "$OUT/plasmashell.log" | grep -v -i "fontconfig" | head -n 120 || true

[ "$fail" = 0 ] || exit 1
echo "plasma image checks passed"
