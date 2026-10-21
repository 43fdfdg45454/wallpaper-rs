#!/usr/bin/env bash
# Inner part of plasma-test.sh, run as the unprivileged test user.
# Usage: plasma-session.sh <work-dir> <output-dir>
set -euo pipefail

WORK=$1
OUT=$2

# Run everything inside a private session bus.
if [ -z "${DBUS_SESSION_BUS_ADDRESS:-}" ]; then
  export XDG_RUNTIME_DIR=/tmp/runtime-$UID
  rm -rf "$XDG_RUNTIME_DIR"
  mkdir -m 700 "$XDG_RUNTIME_DIR"
  exec dbus-run-session -- bash "$0" "$@"
fi

export HOME
HOME=$(getent passwd "$(id -un)" | cut -d: -f6)

cd "$WORK"
tar xzf release.tar.gz
cd wallpaper-rs-*-linux
./install.sh 2>&1 | tee "$OUT/install.log"
cd "$WORK"

pids=()
cleanup() {
  for p in "${pids[@]}"; do kill "$p" 2>/dev/null || true; done
}
trap cleanup EXIT

wait_for() { # <seconds> <command...>
  local t=$1
  shift
  for _ in $(seq $((t * 10))); do
    if "$@" >/dev/null 2>&1; then return 0; fi
    sleep 0.1
  done
  echo "timed out waiting for: $*" >&2
  return 1
}

bus_has() {
  dbus-send --session --print-reply --dest=org.freedesktop.DBus /org/freedesktop/DBus \
    org.freedesktop.DBus.ListNames | grep -q "\"$1\""
}

pipewire >"$OUT/pipewire.log" 2>&1 & pids+=($!)
wait_for 15 test -S "$XDG_RUNTIME_DIR/pipewire-0"
wireplumber >"$OUT/wireplumber.log" 2>&1 & pids+=($!)

# Headless compositor with software rendering.
export KWIN_COMPOSE=Q
kwin_wayland --virtual --width 1280 --height 720 --socket wayland-test \
  >"$OUT/kwin.log" 2>&1 & pids+=($!)
wait_for 30 test -S "$XDG_RUNTIME_DIR/wayland-test"
export WAYLAND_DISPLAY=wayland-test QT_QPA_PLATFORM=wayland XDG_SESSION_TYPE=wayland
export XDG_CURRENT_DESKTOP=KDE KDE_FULL_SESSION=true KDE_SESSION_VERSION=6
export QT_QUICK_BACKEND=software

RUST_LOG=debug "$HOME/.local/bin/wprsd" >"$OUT/wprsd.log" 2>&1 & pids+=($!)
wait_for 30 "$HOME/.local/bin/wprs-ctl" state

plasmashell --no-respawn >"$OUT/plasmashell.log" 2>&1 & pids+=($!)
wait_for 60 bus_has org.kde.plasmashell
sleep 5

# Select the plugin the same way the settings dialog does.
script="desktops().forEach(d => {
  d.wallpaperPlugin = 'io.github.wallpaper_rs';
  d.currentConfigGroup = ['Wallpaper', 'io.github.wallpaper_rs', 'General'];
  d.writeConfig('Image', '$WORK/green.png');
  d.writeConfig('FillMode', 'fill');
  d.reloadConfig();
});"
dbus-send --session --print-reply --dest=org.kde.plasmashell /PlasmaShell \
  org.kde.PlasmaShell.evaluateScript "string:$script" >"$OUT/evaluate.log" 2>&1

# Give the plugin time to request the stream and show the first frame.
sleep 15
"$HOME/.local/bin/wprs-ctl" state | tee "$OUT/state.json"

spectacle --background --nonotify --fullscreen --output "$OUT/screenshot.png" \
  >"$OUT/spectacle.log" 2>&1 || true
wait_for 20 test -s "$OUT/screenshot.png"

pixel() { magick "$OUT/screenshot.png" -format "%[fx:int(255*p{$1,$2}.r)],%[fx:int(255*p{$1,$2}.g)],%[fx:int(255*p{$1,$2}.b)]" info:; }
fail=0
for xy in "640 360" "100 100" "1180 100"; do
  # shellcheck disable=SC2086
  rgb=$(pixel $xy)
  echo "pixel ($xy): $rgb"
  IFS=, read -r r g b <<<"$rgb"
  if [ "$r" -gt 40 ] || [ "$g" -lt 200 ] || [ "$b" -gt 40 ]; then fail=1; fi
done

grep -i "wallpaper-rs\|wallpaper_rs\|kpipewire\|pipewire" "$OUT/plasmashell.log" | tail -n 40 || true

if [ "$fail" = 1 ]; then
  echo "FAIL: the desktop does not show the wallpaper"
  exit 1
fi
echo "plasma test passed"
