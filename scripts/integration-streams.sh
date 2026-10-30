#!/usr/bin/env bash
set -euo pipefail
dbus-monitor --session "type='signal',interface='io.github.wallpaper_rs.Daemon1',member='StreamChanged'" \
  >"$WORK/resized.log" 2>&1 & pids+=($!)
read -r _ _ _ large < <(ctl stream TEST-14 320 180)
sleep 0.5
ctl screen TEST-14 --size 160x90
expect "s['outputs']['TEST-14']['width'], s['outputs']['TEST-14']['height']" "(160, 90)"
resized() { grep -A1 "member=StreamChanged" "$WORK/resized.log" | grep -q 'string "TEST-14"'; }
wait_for 10 resized || fail "no StreamChanged signal for the reduced stream: $(cat "$WORK/resized.log")"
read -r _ _ _ small < <(ctl stream TEST-14 320 180)
[ "$small" != "$large" ] || fail "a stream larger than its screen was kept"
"$BIN/probe" "$small" 10 | grep -q "frame 160x90" || fail "the reduced stream: $("$BIN/probe" "$small" 10)"
ctl screen TEST-14 --size 640x360
expect "s['outputs']['TEST-14']['width'], s['outputs']['TEST-14']['height']" "(320, 180)"
ctl release TEST-14 && expect "list(s['outputs'])" "[]"
echo "stream size limited to the screen checks passed"

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

retry() { for _ in $(seq "$1"); do "${@:2}" && return 0; sleep 0.5; done; return 1; }
bus() {
  local iface=$1; shift
  busctl --user --json=short call io.github.wallpaper_rs.Daemon /io/github/wallpaper_rs/Daemon \
    "io.github.wallpaper_rs.$iface" "$@"
}
out() { python3 -c "import json, sys; d = json.load(sys.stdin)['data']; print(eval(sys.argv[1]))" "$1"; }
reply() { bus "${@:2}" | python3 -c "import json, sys; r = json.loads(json.load(sys.stdin)['data'][0]); print(eval(sys.argv[1]))" "$1"; }
finished() { [ "$(reply "r['job']['state']" Daemon1 GetJob t "$1")" = "done" ]; }
idle() { [ "$(reply "len([j for j in r['jobs'] if j['state'] in ('queued', 'running')])" Daemon1 ListJobs)" = 0 ]; }
signalled() { grep -A4 "member=$1" "$WORK/library-signals.log" | grep -q "$2"; }
refused() { if bus "${@:2}" >"$WORK/refused.out" 2>&1; then fail "accepted: $*"; fi; grep -q "$1" "$WORK/refused.out" || fail "$*: $(cat "$WORK/refused.out")"; }
denied() {
  if dbus-send --session --print-reply --dest=io.github.wallpaper_rs.Daemon /io/github/wallpaper_rs/Daemon \
    "io.github.wallpaper_rs.$2" "${@:3}" >"$WORK/denied.out" 2>&1; then fail "accepted: $*"; fi
  grep -q "$1" "$WORK/denied.out" || fail "$*: $(cat "$WORK/denied.out")"
}
