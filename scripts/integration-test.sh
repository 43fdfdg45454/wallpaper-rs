#!/usr/bin/env bash
set -euo pipefail

BIN=$(realpath "${1:?usage: $0 <bin-dir>}")
ROOT=$(cd "$(dirname "$0")/.." && pwd)
WORK=$(mktemp -d /tmp/wprs-it.XXXXXX)
export XDG_RUNTIME_DIR=$WORK/run XDG_CONFIG_HOME=$WORK/config
mkdir -p "$XDG_RUNTIME_DIR" "$XDG_CONFIG_HOME"
chmod 700 "$XDG_RUNTIME_DIR"

pids=()
cleanup() {
  for p in "${pids[@]}"; do kill "$p" 2>/dev/null || true; done
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

eval "$(dbus-launch --sh-syntax)"
pipewire >"$WORK/pipewire.log" 2>&1 & pids+=($!)
wait_for 10 test -S "$XDG_RUNTIME_DIR/pipewire-0"
wireplumber >"$WORK/wireplumber.log" 2>&1 & pids+=($!)
RUST_LOG=${WPRS_IT_LOG:-debug},naga=warn,wgpu_core=warn,wgpu_hal=warn "$BIN/wprsd" >"$WORK/wprsd.log" 2>&1 & pids+=($!)
wait_for 30 "$BIN/wprs-ctl" state

read -r _ node _ serial < <("$BIN/wprs-ctl" stream TEST-1 320 180)
echo "stream: node $node serial $serial"
[ "$serial" -gt 0 ] || { echo "FAIL: object serial not resolved"; exit 1; }

"$BIN/wprs-ctl" set "$ROOT/tests/fixtures/green-64x32.png" --fit fit
sleep 1
state=$("$BIN/wprs-ctl" state)
echo "state: $state"
grep -q '"TEST-1"' <<<"$state" || { echo "FAIL: output missing from state"; exit 1; }

result=$("$BIN/probe" "$serial" 10)
echo "probe: $result"
grep -q "frame 320x180" <<<"$result" || { echo "FAIL: unexpected frame size"; exit 1; }
grep -q "center bgra \[0, 255, 0, 255\]" <<<"$result" || { echo "FAIL: unexpected center color"; exit 1; }

result=$("$BIN/probe" "node:$node" 10)
echo "probe by node id: $result"
grep -q "center bgra \[0, 255, 0, 255\]" <<<"$result" || { echo "FAIL: connecting by node id"; exit 1; }

read -r _ _ _ serial_again < <("$BIN/wprs-ctl" stream TEST-1 320 180)
[ "$serial_again" = "$serial" ] || { echo "FAIL: same size created a new stream"; exit 1; }

read -r _ _ _ serial2 < <("$BIN/wprs-ctl" stream TEST-1 640 360)
[ "$serial2" != "$serial" ] || { echo "FAIL: resize kept the old stream"; exit 1; }
result=$("$BIN/probe" "$serial2" 10)
grep -q "frame 640x360" <<<"$result" || { echo "FAIL: resized frame: $result"; exit 1; }

ctl() { "$BIN/wprs-ctl" "$@"; }
field() { ctl state | python3 -c "import json, sys; s = json.load(sys.stdin); print(eval(sys.argv[1]))" "$1"; }
expect() { [ "$(field "$1")" = "$2" ] || { echo "FAIL: $1 is $(field "$1"), expected $2"; exit 1; }; }
output="s['outputs']['TEST-1']"

ctl set "$ROOT/tests/fixtures/magenta-64x32.webm" --fit stretch
sleep 1.5
expect "${output}['kind']" video
expect "${output}['playing']" True
result=$("$BIN/probe" "$serial2" 10)
echo "video probe: $result"
read -r b g r _ < <(sed 's/.*center bgra \[\(.*\)\]/\1/; s/,/ /g' <<<"$result")
for pair in "$r 208" "$g 32" "$b 96"; do
  read -r got want <<<"$pair"
  [ $((got > want ? got - want : want - got)) -le 24 ] || { echo "FAIL: video color $r,$g,$b"; exit 1; }
done

ctl pause && expect "${output}['playing']" False && expect "s['paused']" True
ctl resume && sleep 0.3 && expect "${output}['playing']" True
ctl covered TEST-1 true && sleep 0.3 && expect "${output}['active']" False
ctl config pause.when_covered false && sleep 0.3 && expect "${output}['active']" True
ctl config pause.when_covered true && ctl covered TEST-1 false
if ctl config playback.max_fps 0 2>/dev/null; then echo "FAIL: invalid setting accepted"; exit 1; fi
ctl volume 30 && expect "s['volume']" 0.3 && expect "s['muted']" False
ctl mute && expect "s['muted']" True
ctl release TEST-1 && expect "list(s['outputs'])" "[]"
echo "video, pause, settings and release checks passed"

status=0
echo "integration test passed"
