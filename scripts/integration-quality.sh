#!/usr/bin/env bash
set -euo pipefail
declare config daemon log
kill "$daemon"
wait "$daemon" 2>/dev/null || true
bus_conf "$WORK/power.conf"
{ read -r power_bus; read -r power_pid; } < <(dbus-daemon --config-file="$WORK/power.conf" --fork --print-address=1 --print-pid=1)
pids+=("$power_pid")
mkfifo "$WORK/power"
DBUS_SYSTEM_BUS_ADDRESS=$power_bus "$BIN/examples/upower" <"$WORK/power" >"$WORK/upower.out" 2>&1 & pids+=($!)
exec 8>"$WORK/power"
wait_for 10 grep -q ready "$WORK/upower.out" || fail "the power service did not start: $(cat "$WORK/upower.out")"
DBUS_SYSTEM_BUS_ADDRESS=$power_bus RUST_LOG=$log "$BIN/wprsd" >"$WORK/quality.log" 2>&1 & daemon=$!; pids+=("$daemon")
wait_for 30 ctl state || fail "the daemon on battery did not start"
expect "s['on_battery']" True
ctl config pause.on_battery false
read -r _ _ _ watched < <(ctl stream TEST-Q 160 90)
"$BIN/probe" "$watched" 120 --hold >/dev/null 2>&1 & held=$!; pids+=("$held")
ctl lib import "$WORK/scene/glow.wpak" --wait >/dev/null
glow=$(ctl lib search "" --json | python3 -c "import json, sys
print([i['uid'] for i in json.load(sys.stdin)['items'] if (i['package'] or {}).get('id') == 'org.example.glow'][0])")
ctl lib apply "$glow" -o TEST-Q
level="s['outputs']['TEST-Q']['quality'], s['outputs']['TEST-Q']['quality_reason']"
expect "s['outputs']['TEST-Q']['kind']" scene
auto=$(field "$level")
case $auto in *user*) fail "the automatic level says the user chose it: $auto" ;; esac
ctl quality q3 | grep "^TEST-Q  *q3 " >/dev/null || fail "wprs-ctl quality q3: $(ctl quality)"
expect "$level" "('q3', 'user')"
grep -q '^quality = "q3"' "$config" || fail "the level was not saved: $(cat "$config")"
echo ac >&8 && echo battery >&8
sleep 1
expect "s['on_battery'], $level" "(True, 'q3', 'user')"
case $auto in "('q0'"* | "('q1'"*) floored="'q2', 'user'" ;; *) floored=${auto#(} floored=${floored%)} ;; esac
ctl config playback.quality min-q2
expect "$level" "($floored)"
ctl quality q1 -o TEST-Q >/dev/null
expect "$level, s['outputs']['TEST-Q']['quality_choice']" "('q1', 'user', 'q1')"
grep -q '^\[outputs.TEST-Q\]' "$config" || fail "the own level of the screen was not saved: $(cat "$config")"
if ctl quality q7 -o TEST-Q 2>/dev/null; then fail "an invalid level was accepted"; fi
ctl config playback.quality.TEST-Q none
expect "$level, s['outputs']['TEST-Q']['quality_choice']" "($floored, None)"
ctl quality auto >/dev/null
expect "$level" "$auto"
[ "$(ctl quality --json | python3 -c "import json, sys; print(json.load(sys.stdin)['quality'])")" = auto ] ||
  fail "wprs-ctl quality --json: $(ctl quality --json)"
kill "$held" 2>/dev/null || true
ctl release TEST-Q
echo "quality choice checks passed ($auto on battery)"
