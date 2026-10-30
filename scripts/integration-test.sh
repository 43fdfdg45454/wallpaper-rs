#!/usr/bin/env bash
set -euo pipefail

BIN=$(realpath "${1:?usage: $0 <bin-dir>}")
ROOT=$(cd "$(dirname "$0")/.." && pwd)
WORK=$(mktemp -d /tmp/wprs-it.XXXXXX)
export XDG_RUNTIME_DIR=$WORK/run XDG_CONFIG_HOME=$WORK/config XDG_DATA_HOME=$WORK/data XDG_CACHE_HOME=$WORK/cache XDG_STATE_HOME=$WORK/state
mkdir -p "$XDG_RUNTIME_DIR" "$XDG_CONFIG_HOME"
chmod 700 "$XDG_RUNTIME_DIR"

pids=()
cleanup() {
  for p in "${pids[@]}"; do kill -CONT "$p" 2>/dev/null || true; kill "$p" 2>/dev/null || true; done
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
  for _ in $(seq 20); do [ "$(field "$1")" = "$2" ] && return; sleep 0.1; done
  echo "FAIL: $1 is $(field "$1"), expected $2" && exit 1
}
fail() { echo "FAIL: $*"; exit 1; }

bus_conf() {
  printf '<busconfig><type>session</type><listen>unix:dir=%s</listen><auth>EXTERNAL</auth><policy context="default">
    <allow send_destination="*" eavesdrop="true"/><allow eavesdrop="true"/><allow own="*"/></policy></busconfig>' "$WORK" >"$1"
}
bus_conf "$WORK/session.conf"
{ read -r DBUS_SESSION_BUS_ADDRESS; read -r DBUS_SESSION_BUS_PID; } < <(dbus-daemon --config-file="$WORK/session.conf" --fork --print-address=1 --print-pid=1)
export DBUS_SESSION_BUS_ADDRESS DBUS_SESSION_BUS_PID
pipewire >"$WORK/pipewire.log" 2>&1 & pids+=($!)
wait_for 10 test -S "$XDG_RUNTIME_DIR/pipewire-0"
wireplumber >"$WORK/wireplumber.log" 2>&1 & pids+=($!)
log=${WPRS_IT_LOG:-debug},naga=warn,wgpu_core=warn,wgpu_hal=warn

source "$(dirname "$0")"/integration-video.sh
source "$(dirname "$0")"/integration-streams.sh
source "$(dirname "$0")"/integration-library.sh
source "$(dirname "$0")"/integration-playlists.sh
source "$(dirname "$0")"/integration-repos.sh
source "$(dirname "$0")"/integration-scenes.sh
source "$(dirname "$0")"/integration-inputs.sh
source "$(dirname "$0")"/integration-audio.sh
source "$(dirname "$0")"/integration-sound.sh
source "$(dirname "$0")"/integration-recovery.sh

kill "$daemon"
wait "$daemon" 2>/dev/null || true
WPRS_RENDER_STOP=1 RUST_LOG=$log "$BIN/wprsd" >"$WORK/stopped.log" 2>&1 & daemon=$!; pids+=("$daemon")
wait_for 30 ctl state || fail "the daemon did not start again"
sleep 1
kill -USR2 "$daemon"
guarded() { ! ctl pause 2>"$WORK/guard.out" && grep -q "the renderer stopped after an error; restart wprsd" "$WORK/guard.out"; }
wait_for 10 guarded || fail "a change was accepted without a renderer: $(cat "$WORK/guard.out")"
if ctl set "$ROOT/tests/fixtures/green-64x32.png" 2>/dev/null; then fail "a wallpaper was set without a renderer"; fi
{ ctl lib search "" && ctl playlist ls && ctl provider ls && ctl state; } >/dev/null || fail "the library stopped with the renderer"
kill -0 "$daemon" || fail "the daemon exited with its renderer"
echo "renderer guard checks passed"

status=0
echo "integration test passed"
