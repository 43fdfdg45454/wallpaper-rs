#!/usr/bin/env bash
set -euo pipefail

WORK=$1
OUT=$2

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

wait_for() {
  local seconds=$1
  shift
  for _ in $(seq $((seconds * 10))); do
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

export QT_FORCE_STDERR_LOGGING=1 LIBGL_ALWAYS_SOFTWARE=1

debug_run() {
  local log=$1
  shift
  if command -v gdb >/dev/null 2>&1; then
    gdb -q -batch -ex "set debuginfod enabled off" -ex "set pagination off" -ex "handle SIGPIPE nostop noprint" \
      -ex run -ex "bt 40" -ex "thread apply all bt 12" --args "$@" >"$log" 2>&1
  else
    "$@" >"$log" 2>&1
  fi
}

start_kwin() {
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
dbus-update-activation-environment WAYLAND_DISPLAY QT_QPA_PLATFORM XDG_SESSION_TYPE \
  XDG_CURRENT_DESKTOP KDE_FULL_SESSION KDE_SESSION_VERSION LIBGL_ALWAYS_SOFTWARE \
  QT_FORCE_STDERR_LOGGING

RUST_LOG=info,wgpu_hal=debug,wgpu_core=info debug_run "$OUT/wprsd.log" "$HOME/.local/bin/wprsd" & pids+=($!)
wait_for 30 "$HOME/.local/bin/wprs-ctl" state
"$HOME/.local/bin/wprs-ctl" set "$WORK/green.png" --fit fill

bash "$WORK/plasma-config.sh"

dbus-monitor --session "interface='io.github.wallpaper_rs.Daemon1'" \
  >"$OUT/dbus-monitor.log" 2>&1 & pids+=($!)

export QT_LOGGING_RULES="*.debug=true;qt.*.debug=false;kf.kio.*.debug=false;kf.config.*.debug=false"
debug_run "$OUT/plasmashell.log" plasmashell --no-respawn & pids+=($!)
wait_for 60 bus_has org.kde.plasmashell

export -f wait_for bus_has
bash "$WORK/plasma-checks.sh" "$OUT"
bash "$WORK/plasma-features.sh" "$WORK" "$OUT"
