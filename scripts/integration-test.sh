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

ctl() { "$BIN/wprs-ctl" "$@"; }
field() { ctl state | python3 -c "import json, sys; s = json.load(sys.stdin); print(eval(sys.argv[1]))" "$1"; }
expect() {
  for _ in $(seq 20); do [ "$(field "$1")" = "$2" ] && return; sleep 0.1; done
  echo "FAIL: $1 is $(field "$1"), expected $2" && exit 1
}
fail() { echo "FAIL: $*"; exit 1; }

eval "$(dbus-launch --sh-syntax)"
pipewire >"$WORK/pipewire.log" 2>&1 & pids+=($!)
wait_for 10 test -S "$XDG_RUNTIME_DIR/pipewire-0"
wireplumber >"$WORK/wireplumber.log" 2>&1 & pids+=($!)
log=${WPRS_IT_LOG:-debug},naga=warn,wgpu_core=warn,wgpu_hal=warn

broken=$WORK/broken/config.toml
mkdir -p "$(dirname "$broken")"
printf '[playback\nvolume = 0.5\n' >"$broken"
RUST_LOG=$log "$BIN/wprsd" --config "$broken" >"$WORK/broken.log" 2>&1 & broken_pid=$!; pids+=("$broken_pid")
wait_for 30 ctl state
expect "s['config_error'] is not None" True
if ctl reload 2>/dev/null; then fail "reload accepted a broken configuration"; fi
expect "s['config_error'] is not None" True
ctl config pause.on_battery false
expect "s['config_error']" None
grep -qx '\[playback' "$broken.broken" || fail "the broken configuration was not kept"
grep -q 'on_battery = false' "$broken" || fail "the new configuration was not saved"
[ "$(stat -c %a "$broken")" = 600 ] || fail "config.toml is not private"
[ "$(stat -c %a "$broken.broken")" = 600 ] || fail "config.toml.broken is not private"
kill "$broken_pid"
wait "$broken_pid" || true
echo "broken configuration checks passed"

RUST_LOG=$log "$BIN/wprsd" >"$WORK/wprsd.log" 2>&1 & pids+=($!)
for _ in $(seq 100); do dbus-send --session --dest=org.freedesktop.DBus / org.freedesktop.DBus.GetId & done
wait_for 30 ctl state || fail "the daemon did not start while clients came and went"
timeout 30 "$BIN/wprsd" >"$WORK/second.log" 2>&1 || true
grep -q "another wprsd" "$WORK/second.log" || { echo "FAIL: a second daemon took over"; exit 1; }
dbus-send --session --print-reply --dest=org.freedesktop.DBus /org/freedesktop/DBus \
  org.freedesktop.DBus.RequestName string:io.github.wallpaper_rs.Daemon uint32:6 | grep -q "uint32 3" ||
  fail "another client replaced the daemon"

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

output="s['outputs']['TEST-1']"

consumers=()
consume() { "$BIN/probe" "$1" 120 --hold >"$WORK/consumer-$1.out" 2>&1 & consumers+=($!); pids+=($!); }
leave() { kill "${consumers[@]}"; wait "${consumers[@]}" 2>/dev/null || true; consumers=(); }

ctl set "$ROOT/tests/fixtures/magenta-64x32.webm" --fit stretch
expect "${output}['kind']" video
near() { [ $(($1 > $2 ? $1 - $2 : $2 - $1)) -le 24 ]; }
shows_video() {
  read -r b g r _ < <("$BIN/probe" "$serial2" 10 | sed 's/.*center bgra \[\(.*\)\]/\1/; s/,/ /g')
  near "$r" 208 && near "$g" 32 && near "$b" 96
}
wait_for 5 shows_video || { echo "FAIL: video frame $("$BIN/probe" "$serial2" 10)"; exit 1; }
expect "${output}['playing']" False
consume "$serial2"
expect "${output}['playing']" True

ctl pause && expect "${output}['playing']" False && expect "s['paused']" True
ctl resume && expect "${output}['playing']" True
ctl covered TEST-1 true && expect "${output}['active']" False
ctl config pause.when_covered false && expect "${output}['active']" True
ctl config pause.when_covered true && ctl covered TEST-1 false
if ctl config playback.max_fps 0 2>/dev/null; then echo "FAIL: invalid setting accepted"; exit 1; fi
ctl volume 30 && expect "s['volume']" 0.3 && expect "s['muted']" False
ctl mute && expect "s['muted']" True
leave && expect "${output}['playing']" False
ctl release TEST-1 && expect "list(s['outputs'])" "[]"
echo "video, pause, settings and release checks passed"

read -r _ _ _ left < <(ctl stream TEST-5 160 90)
read -r _ _ _ right < <(ctl stream TEST-6 160 90)
ctl set "$ROOT/tests/fixtures/magenta-64x32.webm"
consume "$left" && consume "$right"
expect "s['outputs']['TEST-5']['playing']" True
expect "s['outputs']['TEST-6']['playing']" True
ctl covered TEST-5 true
expect "s['outputs']['TEST-5']['playing']" False
expect "s['outputs']['TEST-6']['playing']" True
leave
ctl release TEST-5 && ctl release TEST-6 && expect "list(s['outputs'])" "[]"
echo "one video on two outputs, one of them covered, checks passed"

if ctl config no.such.setting >"$WORK/unknown.out" 2>&1; then fail "an unknown setting was accepted"; fi
grep -q "unknown setting" "$WORK/unknown.out" || fail "unknown setting message: $(cat "$WORK/unknown.out")"
if ctl stream TEST-4 8200 8200 2>/dev/null; then fail "a stream larger than the GPU buffers was accepted"; fi
head -c 65536 /dev/urandom >"$WORK/noise.webm"
if ctl set "$WORK/noise.webm" 2>/dev/null; then fail "a file of random bytes was accepted as a video"; fi
if ctl set http://127.0.0.1:9/x.webm >"$WORK/remote.out" 2>&1; then fail "a URL was accepted"; fi
grep -q "not a local file" "$WORK/remote.out" || fail "URL message: $(cat "$WORK/remote.out")"
dbus-send --session --print-reply --dest=io.github.wallpaper_rs.Daemon /io/github/wallpaper_rs/Daemon \
  io.github.wallpaper_rs.Daemon1.SetWallpaper string:http://127.0.0.1:9/x.webm string: string: \
  >"$WORK/remote-bus.out" 2>&1 || true
grep -q "InvalidArgs" "$WORK/remote-bus.out" || fail "the daemon accepted a URL: $(cat "$WORK/remote-bus.out")"
ctl state >/dev/null
echo "unknown setting, size limit and local file checks passed"

read -r _ _ _ held < <(ctl stream " " 160 90)
sleep 0.5
expect "list(s['outputs'])" "['default']"
"$BIN/probe" "$held" 10 | grep -q "frame 160x90" || fail "the held stream did not survive the command"
ctl covered "" true && expect "s['outputs']['default']['covered']" True
ctl release "" && expect "list(s['outputs'])" "[]"
echo "held stream and default output name checks passed"

"$BIN/wprs-ctl" stream TEST-2 320 180 --wait >"$WORK/first.out" 2>&1 & first=$!; pids+=("$first")
"$BIN/wprs-ctl" stream TEST-2 320 180 --wait >"$WORK/second.out" 2>&1 & second=$!; pids+=("$second")
for n in 1 2 3 4 5; do
  dbus-send --session --type=method_call --dest=io.github.wallpaper_rs.Daemon /io/github/wallpaper_rs/Daemon \
    io.github.wallpaper_rs.Daemon1.RequestStream string:GONE-$n uint32:64 uint32:64
done
wait_for 10 grep -q serial "$WORK/first.out" || fail "first claim: $(cat "$WORK/first.out")"
wait_for 10 grep -q serial "$WORK/second.out" || fail "second claim: $(cat "$WORK/second.out")"
read -r _ _ _ shared <"$WORK/first.out"
expect "s['outputs']['TEST-2']['clients']" 2
kill "$second"
wait "$second" || fail "stream --wait did not release its claim cleanly"
expect "s['outputs']['TEST-2']['clients']" 1
"$BIN/probe" "$shared" 10 | grep -q "frame 320x180" || fail "the stream did not survive one release"
kill -KILL "$first"
wait "$first" 2>/dev/null || true
expect "list(s['outputs'])" "[]"
echo "shared stream claim checks passed"

ctl stream TEST-3 160 90 >/dev/null
ctl set "$ROOT/tests/fixtures/green-64x32.png" -o TEST-3 --fit fit
ctl fit center && expect "s['outputs']['TEST-3']['fit']" center
expect "s['outputs']['TEST-3']['image'].endswith('green-64x32.png')" True
ctl fit tile -o TEST-3 && expect "s['outputs']['TEST-3']['fit']" tile
expect "s['outputs']['TEST-3']['image'].endswith('green-64x32.png')" True
if ctl background red 2>/dev/null; then fail "an invalid color was accepted"; fi
ctl background '#102030' -o TEST-3 && expect "s['outputs']['TEST-3']['background']" '#102030'
cp "$ROOT/tests/fixtures/green-64x32.png" "$WORK/green #1.png"
ctl set "file://$WORK/green%20%231.png" -o TEST-3
expect "s['outputs']['TEST-3']['image'].endswith('/green #1.png')" True
ctl release TEST-3 && expect "list(s['outputs'])" "[]"
echo "fit, background and file URI checks passed"

restored() {
  read -r _ _ _ serial < <(ctl stream TEST-1 320 180) || return 1
  "$BIN/probe" "$serial" 3 | grep -q "frame 320x180 .* buffers 4 center bgra \[0, 255, 0, 255\]"
}
retry() { for _ in $(seq "$1"); do "${@:2}" && return 0; sleep 0.5; done; return 1; }
ctl stream TEST-1 320 180 >/dev/null
ctl set "$ROOT/tests/fixtures/green-64x32.png" --fit fit
retry 10 restored || { echo "FAIL: stream before the PipeWire restart"; exit 1; }
dbus-monitor --session "type='signal',interface='io.github.wallpaper_rs.Daemon1',member='StreamChanged'" \
  >"$WORK/signals.log" 2>&1 & pids+=($!)
sleep 0.5
kill "${pids[0]}" "${pids[1]}"
wait "${pids[0]}" "${pids[1]}" 2>/dev/null || true
wait_for 5 test ! -S "$XDG_RUNTIME_DIR/pipewire-0"
pipewire >"$WORK/pipewire-restarted.log" 2>&1 & pids+=($!)
wait_for 10 test -S "$XDG_RUNTIME_DIR/pipewire-0"
wireplumber >"$WORK/wireplumber-restarted.log" 2>&1 & pids+=($!)
retry 30 restored || { echo "FAIL: stream not restored after restarting PipeWire"; exit 1; }
announced() { grep -A1 "member=StreamChanged" "$WORK/signals.log" | grep -q 'string "TEST-1"'; }
wait_for 10 announced || fail "no StreamChanged signal for TEST-1: $(cat "$WORK/signals.log")"
echo "PipeWire restart recovery and StreamChanged passed (serial $serial)"

status=0
echo "integration test passed"
