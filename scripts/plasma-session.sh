#!/usr/bin/env bash
# Inner part of plasma-test.sh, run as the unprivileged test user.
# Usage: plasma-session.sh <work-dir> <output-dir> <x-display>
set -euo pipefail

WORK=$1
OUT=$2
X_DISPLAY=$3

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
  local rc=$?
  for p in "${pids[@]}"; do kill "$p" 2>/dev/null || true; done
  if [ "$rc" != 0 ]; then
    for f in "$OUT"/*.log; do
      echo "===== $(basename "$f") (last 60 lines)"
      tail -n 60 "$f"
    done
  fi
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

# KDE programs log to the journal by default; there is none here.
export QT_FORCE_STDERR_LOGGING=1
# Software OpenGL (llvmpipe) for every client.
export LIBGL_ALWAYS_SOFTWARE=1

# Runs a program under gdb when available so crashes leave a backtrace.
debug_run() { # <log> <program> [args...]
  local log=$1
  shift
  if command -v gdb >/dev/null 2>&1; then
    gdb -q -batch -ex "set debuginfod enabled off" -ex "set pagination off" -ex "handle SIGPIPE nostop noprint" \
      -ex run -ex "thread apply all bt 25" --args "$@" >"$log" 2>&1
  else
    "$@" >"$log" 2>&1
  fi
}

# The X server is started by plasma-test.sh.
export DISPLAY=$X_DISPLAY QT_QPA_PLATFORM=xcb XDG_SESSION_TYPE=x11
unset WAYLAND_DISPLAY
export XDG_CURRENT_DESKTOP=KDE KDE_FULL_SESSION=true KDE_SESSION_VERSION=6
# Services started by D-Bus activation (activity manager, the daemon) need
# the same graphical environment.
dbus-update-activation-environment DISPLAY QT_QPA_PLATFORM XDG_SESSION_TYPE \
  XDG_CURRENT_DESKTOP KDE_FULL_SESSION KDE_SESSION_VERSION LIBGL_ALWAYS_SOFTWARE \
  QT_FORCE_STDERR_LOGGING

# The daemon is started by D-Bus activation, as on a real desktop.
wait_for 30 "$HOME/.local/bin/wprs-ctl" state

# Start from an existing session in which the wallpaper-rs plugin is already
# selected, as after choosing it in "Configure Desktop and Wallpaper". This
# also keeps plasmashell from running its first-start layout script and
# pending update scripts, which need a full graphical session.
activity=6f1d2a0e-3b7c-4e58-9a41-0c2d5e7f8a90
mkdir -p "$HOME/.config"
cat >"$HOME/.config/kactivitymanagerdrc" <<CONF
[activities]
$activity=Default

[main]
currentActivity=$activity
CONF
cat >"$HOME/.config/plasma-org.kde.plasma.desktop-appletsrc" <<CONF
[Containments][1]
activityId=$activity
formfactor=0
immutability=1
lastScreen=0
location=0
plugin=org.kde.plasma.folder
wallpaperplugin=io.github.wallpaper_rs

[Containments][1][Wallpaper][io.github.wallpaper_rs][General]
FillMode=fill
Image=$WORK/green.png
CONF
updates=$(find /usr/share/plasma/shells/org.kde.plasma.desktop/contents/updates \
  -name '*.js' 2>/dev/null | sort | paste -sd, -)
cat >"$HOME/.config/plasmashellrc" <<CONF
[Updates]
performed=$updates
CONF

# Record every call to the daemon interface.
dbus-monitor --session "interface='io.github.wallpaper_rs.Daemon1'" \
  >"$OUT/dbus-monitor.log" 2>&1 & pids+=($!)

export QT_LOGGING_RULES="qml.debug=true;js.debug=true;kf.plasma.*.debug=true;kpipewire*=true"
debug_run "$OUT/plasmashell.log" plasmashell --no-respawn & pids+=($!)
wait_for 60 bus_has org.kde.plasmashell

# Give the plugin time to request the stream and show the first frame.
sleep 15
"$HOME/.local/bin/wprs-ctl" state | tee "$OUT/state.json"

magick import -display "$X_DISPLAY" -window root "$OUT/screenshot.png"

pixel() { magick "$OUT/screenshot.png" -format "%[fx:int(255*p{$1,$2}.r)],%[fx:int(255*p{$1,$2}.g)],%[fx:int(255*p{$1,$2}.b)]" info:; }
fail=0
for xy in "640 360" "100 100" "1180 100"; do
  # shellcheck disable=SC2086
  rgb=$(pixel $xy)
  echo "pixel ($xy): $rgb"
  IFS=, read -r r g b <<<"$rgb"
  if [ "$r" -gt 40 ] || [ "$g" -lt 200 ] || [ "$b" -gt 40 ]; then fail=1; fi
done

echo "===== screenshot colors"
magick "$OUT/screenshot.png" -format %c histogram:info:- | sort -rn | head -n 5
echo "===== X windows"
xwininfo -root -tree 2>&1 | head -n 30 || true
echo "===== RandR outputs"
xrandr --query 2>&1 | head -n 10 || true
echo "===== desktop config written by Plasma"
cat "$HOME/.config/plasma-org.kde.plasma.desktop-appletsrc" || true
echo "===== D-Bus calls to the daemon"
cat "$OUT/dbus-monitor.log" || true
echo "===== plasmashell messages about the plugin"
grep -i -E "wallpaper|qml|kpipewire|pipewire|error|warning" "$OUT/plasmashell.log" | tail -n 80 || true

if [ "$fail" = 1 ]; then
  echo "FAIL: the desktop does not show the wallpaper"
  exit 1
fi
echo "plasma test passed"
