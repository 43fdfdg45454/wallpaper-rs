#!/usr/bin/env bash
set -euo pipefail
[ $# -gt 0 ] || { echo "usage: $0 COMMAND [ARG]..." >&2 && exit 2; }
run=$(mktemp -d)
chmod 700 "$run"
pids=()
stop() {
  [ ${#pids[@]} -eq 0 ] || kill "${pids[@]}" 2>/dev/null || true
  wait 2>/dev/null || true
  rm -rf "$run"
}
trap stop EXIT
export XDG_RUNTIME_DIR=$run PIPEWIRE_RUNTIME_DIR=$run
unset PIPEWIRE_REMOTE PULSE_SERVER
{ read -r DBUS_SESSION_BUS_ADDRESS; read -r bus; } < <(dbus-daemon --session --fork --print-address=1 --print-pid=1)
export DBUS_SESSION_BUS_ADDRESS
pids+=("$bus")
pipewire >"$run/pipewire.log" 2>&1 &
pids+=($!)
for _ in $(seq 100); do [ -S "$run/pipewire-0" ] && break; sleep 0.1; done
[ -S "$run/pipewire-0" ] || { echo "the private PipeWire did not start" >&2 && cat "$run/pipewire.log" >&2 && exit 1; }
wireplumber >"$run/wireplumber.log" 2>&1 &
pids+=($!)
pw-cli create-node adapter '{ factory.name=support.null-audio-sink node.name=wprs-quiet-sink
  media.class=Audio/Sink object.linger=true audio.position=[FL FR] }' >/dev/null
"$@"
