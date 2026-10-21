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
  local rc=$?
  for p in "${pids[@]}"; do kill "$p" 2>/dev/null || true; done
  if [ "$rc" != 0 ]; then
    for f in "$OUT"/*.log; do
      echo "===== $(basename "$f") (first 40 lines)"
      head -n 40 "$f"
      if grep -q 'received signal' "$f"; then
        echo "===== $(basename "$f") (crash)"
        grep -m 1 -A 50 'received signal' "$f"
      fi
      echo "===== $(basename "$f") (last 40 lines)"
      tail -n 40 "$f"
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
      -ex run -ex "bt 40" -ex "thread apply all bt 12" --args "$@" >"$log" 2>&1
  else
    "$@" >"$log" 2>&1
  fi
}

# Headless KWin. The virtual backend needs a render node (vgem provides one);
# if it cannot start, fall back to the DRM backend on a vkms virtual monitor.
start_kwin() { # <log> [args...]
  local log=$1
  shift
  debug_run "$log" kwin_wayland --socket wayland-test "$@" & pids+=($!)
  wait_for 20 test -S "$XDG_RUNTIME_DIR/wayland-test"
}
ls -l /dev/dri || true
if ! start_kwin "$OUT/kwin.log" --virtual --width 1280 --height 720; then
  vkms=$(for c in /sys/class/drm/card*-Writeback-*; do basename "${c%%-*}"; done | head -n 1)
  echo "virtual backend failed; trying DRM on /dev/dri/$vkms"
  KWIN_DRM_DEVICES=/dev/dri/$vkms start_kwin "$OUT/kwin-drm.log" --drm
fi
export WAYLAND_DISPLAY=wayland-test QT_QPA_PLATFORM=wayland XDG_SESSION_TYPE=wayland
export XDG_CURRENT_DESKTOP=KDE KDE_FULL_SESSION=true KDE_SESSION_VERSION=6
# Services started by D-Bus activation (activity manager, the daemon) need
# the same graphical environment.
dbus-update-activation-environment WAYLAND_DISPLAY QT_QPA_PLATFORM XDG_SESSION_TYPE \
  XDG_CURRENT_DESKTOP KDE_FULL_SESSION KDE_SESSION_VERSION LIBGL_ALWAYS_SOFTWARE \
  QT_FORCE_STDERR_LOGGING

# Start the daemon directly (under gdb) so a crash leaves a backtrace; on a
# real desktop it is started by systemd or D-Bus activation instead.
RUST_LOG=info,wgpu_hal=debug,wgpu_core=info debug_run "$OUT/wprsd.log" "$HOME/.local/bin/wprsd" & pids+=($!)
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

export QT_LOGGING_RULES="*.debug=true;qt.*.debug=false;kf.kio.*.debug=false;kf.config.*.debug=false"
debug_run "$OUT/plasmashell.log" plasmashell --no-respawn & pids+=($!)
wait_for 60 bus_has org.kde.plasmashell

# Give the plugin time to request the stream and show the first frame.
sleep 15
"$HOME/.local/bin/wprs-ctl" state | tee "$OUT/state.json"

# The plugin's KPipeWire stream must reach the streaming state; a stream
# stuck in "paused" never delivers a frame and leaves the desktop black.
echo "===== KPipeWire stream states"
grep -E "kpipewire_logging: (state changed|created successfully|Stream error)" "$OUT/plasmashell.log" || true
fail=0
if ! grep -q "kpipewire_logging: state changed paused -> streaming" "$OUT/plasmashell.log"; then
  echo "FAIL: the wallpaper stream never started streaming in plasmashell"
  fail=1
fi

# A screenshot proves the frames reach the screen, but only when KWin can
# composite in this environment; otherwise it is reported and not enforced.
kwin_renders=1
if grep -q "Failed to find a working output layer configuration" "$OUT"/kwin*.log; then
  kwin_renders=0
  echo "note: KWin cannot composite here; the screenshot check is informational"
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
  pixel() { magick "$OUT/screenshot.png" -format "%[fx:int(255*p{$1,$2}.r)],%[fx:int(255*p{$1,$2}.g)],%[fx:int(255*p{$1,$2}.b)]" info:; }
  read -r shot_w shot_h < <(magick identify -format "%w %h\n" "$OUT/screenshot.png")
  echo "screenshot: ${shot_w}x${shot_h}"
  for xy in "$((shot_w / 2)) $((shot_h / 2))" "$((shot_w / 10)) $((shot_h / 7))" "$((shot_w * 9 / 10)) $((shot_h / 7))"; do
    # shellcheck disable=SC2086
    rgb=$(pixel $xy)
    echo "pixel ($xy): $rgb"
    IFS=, read -r r g b <<<"$rgb"
    if [ "$r" -gt 40 ] || [ "$g" -lt 200 ] || [ "$b" -gt 40 ]; then shot_ok=0; fi
  done
fi
if [ "$shot_ok" = 0 ] && [ "$kwin_renders" = 1 ]; then
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

if [ "$fail" = 1 ]; then
  exit 1
fi
echo "plasma test passed"
