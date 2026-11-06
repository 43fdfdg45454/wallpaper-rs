#!/usr/bin/env bash
set -euo pipefail
[ $# -gt 0 ] || { echo "usage: $0 COMMAND [ARG]..." >&2 && exit 2; }
run=$(mktemp -d)
chmod 700 "$run"
mkdir -p "$run/config" "$run/state" "$run/data" "$run/cache"
real=${XDG_STATE_HOME:-$HOME/.local/state}/wireplumber/restore-stream
here=$PWD/wireplumber
heard() { grep -c wprs-quiet-sink "$1" 2>/dev/null || true; }
before=$(heard "$real")
[ ! -e "$here" ] || { echo "$here is in the way of the leak check" >&2 && exit 1; }
pids=()
stop() {
  local status=$?
  [ ${#pids[@]} -eq 0 ] || kill "${pids[@]}" 2>/dev/null || true
  wait 2>/dev/null || true
  rm -rf "$run"
  [ ! -e "$here" ] || { echo "the private WirePlumber wrote its state into $here" >&2 && status=1; }
  [ "$(heard "$real")" = "$before" ] || { echo "the private WirePlumber wrote its state into $real" >&2 && status=1; }
  exit "$status"
}
trap stop EXIT
own() { env XDG_CONFIG_HOME="$run/config" XDG_STATE_HOME="$run/state" XDG_DATA_HOME="$run/data" XDG_CACHE_HOME="$run/cache" "$@"; }
export XDG_RUNTIME_DIR=$run PIPEWIRE_RUNTIME_DIR=$run
unset PIPEWIRE_REMOTE PULSE_SERVER
{ read -r DBUS_SESSION_BUS_ADDRESS; read -r bus; } < <(dbus-daemon --session --fork --print-address=1 --print-pid=1)
export DBUS_SESSION_BUS_ADDRESS
pids+=("$bus")
own pipewire >"$run/pipewire.log" 2>&1 &
pids+=($!)
for _ in $(seq 100); do [ -S "$run/pipewire-0" ] && break; sleep 0.1; done
[ -S "$run/pipewire-0" ] || { echo "the private PipeWire did not start" >&2 && cat "$run/pipewire.log" >&2 && exit 1; }
own wireplumber >"$run/wireplumber.log" 2>&1 &
pids+=($!)
own pw-cli create-node adapter '{ factory.name=support.null-audio-sink node.name=wprs-quiet-sink
  media.class=Audio/Sink object.linger=true audio.position=[FL FR] }' >/dev/null
"$@"
