#!/usr/bin/env bash
set -euo pipefail
declare anchor log output port
restored() {
  read -r _ _ _ serial < <(ctl stream TEST-1 320 180) || return 1
  [ -z "${1:-}" ] || [ "$serial" = "$1" ] || return 1
  "$BIN/probe" "$serial" 3 | grep -q "frame 320x180 .* buffers 4 center bgra \[0, 255, 0, 255\]"
}
ctl stream TEST-1 320 180 >/dev/null
ctl set "$ROOT/tests/fixtures/green-64x32.png" --fit fit
retry 10 restored || { echo "FAIL: stream before the PipeWire restart"; exit 1; }
old=$serial
dbus-monitor --session "type='signal',interface='io.github.wallpaper_rs.Daemon1',member='StreamChanged'" \
  >"$WORK/signals.log" 2>&1 & pids+=($!)
sleep 0.5
kill "${pids[0]}" "${pids[1]}"
wait "${pids[0]}" "${pids[1]}" 2>/dev/null || true
wait_for 5 test ! -S "$XDG_RUNTIME_DIR/pipewire-0"
pipewire >"$WORK/pipewire-restarted.log" 2>&1 & pids+=($!)
wait_for 10 test -S "$XDG_RUNTIME_DIR/pipewire-0"
wireplumber >"$WORK/wireplumber-restarted.log" 2>&1 & pids+=($!)
announced() { grep -A3 "member=StreamChanged" "$WORK/signals.log" | grep -A2 'string "TEST-1"' | sed -n 's/^ *uint64 //p' | tail -n 1; }
renewed() { [ -n "$(announced)" ]; }
wait_for 30 renewed || fail "no StreamChanged signal for TEST-1: $(cat "$WORK/signals.log")"
new=$(announced)
[ "$new" != "$old" ] || fail "StreamChanged announced the serial $old of the stream before the restart"
retry 30 restored "$new" || fail "TEST-1 is not shown under its new serial $new: $(ctl stream TEST-1 320 180)"
echo "PipeWire restart recovery and StreamChanged passed (serial $old, then $new)"

kill "$daemon"
wait "$daemon" 2>/dev/null || true
WPRS_DMABUF=emulate RUST_LOG=$log "$BIN/wprsd" >"$WORK/dmabuf.log" 2>&1 & daemon=$!; pids+=("$daemon")
wait_for 30 ctl state || fail "the daemon with GPU stream buffers did not start"
read -r _ _ _ gpu < <(ctl stream TEST-1 320 180)
ctl set "$ROOT/tests/fixtures/green-64x32.png" --fit fit
green="buffers 4 center bgra \[0, 255, 0, 255\]"
retry 10 sh -c "'$BIN/probe' $gpu 3 --dmabuf | grep -q 'type DmaBuf modifier 0x0 $green'" ||
  fail "no frame in GPU memory: $("$BIN/probe" "$gpu" 3 --dmabuf 2>&1)"
first=$("$BIN/probe" "$gpu" 3 2>&1)
grep -q "type MemFd $green" <<<"$first" || fail "the first frame in shared memory after GPU memory: $first"
"$BIN/probe" "$gpu" 5 --dmabuf --late | grep -q "type DmaBuf" || fail "a late GPU memory consumer waited"
ctl set "$ROOT/tests/fixtures/magenta-64x32.webm" --fit stretch
"$BIN/probe" "$gpu" 4 --dmabuf --hold >"$WORK/gpu.out" & held=$!
expect "${output}['buffers'], ${output}['kind']" "('dmabuf', 'video')"
wait "$held"
frames=$(sed -n 's/^frames //p' "$WORK/gpu.out")
[ "${frames:-0}" -gt 12 ] || fail "GPU stream buffers were not recycled: $(cat "$WORK/gpu.out")"
echo "GPU stream buffer checks passed ($frames frames through 4 buffers)"

kill "$daemon"
wait "$daemon" 2>/dev/null || true
WPRS_DMABUF=emulate-flaky RUST_LOG=$log "$BIN/wprsd" >"$WORK/flaky.log" 2>&1 & daemon=$!; pids+=("$daemon")
wait_for 30 ctl state || fail "the daemon with failing GPU stream buffers did not start"
read -r _ _ _ gpu < <(ctl stream TEST-1 320 180)
ctl set "$ROOT/tests/fixtures/green-64x32.png" --fit fit && expect "${output}['kind']" image
"$BIN/probe" "$gpu" 4 --dmabuf --hold >"$WORK/flaky.out" 2>&1 & held=$!
expect "${output}['buffers']" memfd
wait "$held" || fail "no frame after a failed GPU allocation: $(cat "$WORK/flaky.out")"
grep -q "type MemFd $green" "$WORK/flaky.out" || fail "the consumer got no shared memory: $(cat "$WORK/flaky.out")"
echo "GPU allocation fallback passed"

kill "$daemon"
wait "$daemon" 2>/dev/null || true
bus_conf "$WORK/system.conf"
{ read -r system; read -r system_pid; } < <(dbus-daemon --config-file="$WORK/system.conf" --fork --print-address=1 --print-pid=1)
pids+=("$system_pid")
mkfifo "$WORK/zone"
DBUS_SYSTEM_BUS_ADDRESS=$system "$BIN/examples/timedate" <"$WORK/zone" >"$WORK/timedate.out" 2>&1 & pids+=($!)
exec 7>"$WORK/zone"
wait_for 10 grep -q ready "$WORK/timedate.out" || fail "the time zone service did not start: $(cat "$WORK/timedate.out")"
TZ=UTC DBUS_SYSTEM_BUS_ADDRESS=$system RUST_LOG=$log "$BIN/wprsd" >"$WORK/robust.log" 2>&1 & daemon=$!; pids+=("$daemon")
wait_for 30 ctl state || fail "the daemon with a system bus did not start"
echo Etc/UTC >&7
clock=$WORK/clock && mkdir -p "$clock"
png "$clock/a.png" ff0000 && png "$clock/b.png" 00ff00 && png "$clock/c.png" 0000ff
clock_source=$(ctl lib add "$clock" --wait | sed -n 1p)
mapfile -t tones < <(ctl lib search "source:$clock_source sort:title" --json | js "'\n'.join(i['uid'] for i in r['items'])")
[ "${#tones[@]}" = 3 ] || fail "the clock pictures: ${tones[*]}"
ctl stream TEST-8 160 90 >/dev/null
ctl playlist new Clock >/dev/null && ctl playlist add Clock "${tones[@]}"
at=$(date -u -d '+30 min' +%H:%M)
ctl playlist rule Clock interval 1h && ctl playlist rule Clock at "$at"
started=$(date +%s)
ctl playlist start Clock -o TEST-8
timed=$(python3 -c "import datetime as d, sys; n = d.datetime.now(d.timezone.utc); h, m = map(int, sys.argv[1].split(':'))
t = n.replace(hour=h, minute=m, second=0, microsecond=0); print(int((t + d.timedelta(days=t <= n)).timestamp()))" "$at")
next_at() { field "s['outputs']['TEST-8']['playlist']['next_at']"; }
[ "$(next_at)" = "$timed" ] || fail "the switch at $at UTC is due at $(next_at), not $timed"
echo Etc/GMT-5 >&7
hourly() { local n; n=$(next_at); [ "$n" -ge $((started + 3600)) ] && [ "$n" -le $((started + 3610)) ]; }
wait_for 10 hourly || fail "after a zone change the next switch is $(next_at), not an hour after $started"
echo Etc/UTC >&7
timely() { [ "$(next_at)" = "$timed" ]; }
wait_for 10 timely || fail "back in the first zone the next switch is $(next_at), not $timed"
echo "time and zone checks passed"
ctl playlist rule Clock resume false
position() { field "s['outputs']['TEST-8']['playlist']['position']"; }
for _ in 1 2 3 4; do
  before=$(position) && ctl playlist start Clock -o TEST-8 && sleep 0.3
  [ "$(position)" = $(((before + 1) % 3)) ] || fail "a start moved the playlist from $before to $(position)"
done
ctl stream TEST-9 160 90 >/dev/null
ctl playlist start Clock
for flag in true false true false; do
  ctl playlist rule Clock synced "$flag" & ruled=$!
  ctl next & stepped=$!
  wait "$ruled" || fail "a rule change failed while the playlist moved"
  wait "$stepped" || fail "a step failed while a rule changed"
done
ctl playlist rule Clock synced true && ctl next
expect "s['outputs']['TEST-9']['item'] == s['outputs']['TEST-8']['item'] in ($(printf "'%s'," "${tones[@]}"))" True
ctl release TEST-9
echo "playlist start and rule race checks passed"
python3 - "$XDG_DATA_HOME/wallpaper-rs/library.db" >"$WORK/locked.out" <<'LOCK' & locker=$!; pids+=("$locker")
import sqlite3, sys, time
db = sqlite3.connect(sys.argv[1], isolation_level=None)
db.execute("BEGIN EXCLUSIVE")
print("locked", flush=True)
time.sleep(8)
db.execute("COMMIT")
LOCK
wait_for 10 grep -q locked "$WORK/locked.out" || fail "the library could not be locked"
ctl lib apply "${tones[2]}" -o TEST-8 || fail "a wallpaper that was shown was reported as failed"
expect "s['outputs']['TEST-8']['item']" "${tones[2]}"
wait "$locker"
grep -q "its use was not recorded" "$WORK/robust.log" || fail "the failed library write was not reported"
echo "a shown wallpaper whose use cannot be recorded passed"
many=$WORK/many && mkdir -p "$many"
for n in $(seq 300); do cp "$ROOT/tests/fixtures/green-64x32.png" "$many/p$n.png" && printf '%s' "$n" >>"$many/p$n.png"; done
many_source=$(ctl lib add "$many" --wait | sed -n 1p)
counted() { ctl lib search "source:$many_source $1" --json | js "r['total']"; }
found() { [ "$(counted "$1")" = 1 ]; }
[ "$(counted "")" = 300 ] || fail "lib add --wait returned before its scan ended: $(counted "")"
ctl lib scan "$many_source" >/dev/null
cp "$ROOT/tests/fixtures/green-64x32.png" "$many/zz-late.png"
wait_for 30 found zz-late || fail "a file written during a scan was lost"
cp "$ROOT/tests/fixtures/green-64x32.png" "$WORK/elsewhere.png" && ln "$WORK/elsewhere.png" "$many/linked.png"
wait_for 30 found linked || fail "a hard link was not noticed"
ctl lib unsource "$many_source"
echo "scan wait and folder watch checks passed"
keyring() { busctl --user status org.freedesktop.secrets >/dev/null 2>&1; }
unlock() {
  printf 'keyring-word' | gnome-keyring-daemon --unlock --components=secrets --foreground >>"$WORK/keyring.log" 2>&1 &
  kr=$! && pids+=("$kr")
  wait_for 10 keyring || fail "the keyring did not start"
}
secretly() { ctl provider ls --json | jq_ "[(p['state'], p['has_secret']) for p in r['providers'] if p['id'] == '$1']"; }
unlock
printf 'first\n' | ctl provider add twin webdav url=https://dav.example.org/a user=me refresh=0s --secret-stdin >/dev/null 2>&1 & one=$!
printf 'second\n' | ctl provider add twin webdav url=https://dav.example.org/a user=me refresh=0s --secret-stdin >/dev/null 2>&1 & two=$!
added=0
if wait "$one"; then added=$((added + 1)); fi
if wait "$two"; then added=$((added + 1)); fi
[ "$added" = 1 ] || fail "$added of two adds of the same provider succeeded"
[ "$(secretly twin)" = "[('idle', True)]" ] || fail "the provider added twice lost its secret: $(secretly twin)"
kill -STOP "$kr"
since=$(date +%s%N)
[ "$(secretly twin)" = "[('idle', True)]" ] || fail "a silent keyring changed the provider: $(secretly twin)"
[ $(($(date +%s%N) - since)) -lt 3000000000 ] || fail "listing providers waited for a silent keyring"
kill -CONT "$kr"
kill "$kr"
wait "$kr" 2>/dev/null || true
keyring && fail "the keyring did not stop"
ctl provider rm twin
unlock
ctl provider add twin webdav url=https://dav.example.org/a user=me refresh=0s --no-secret >/dev/null
[ "$(secretly twin)" = "[('needs_secret', False)]" ] || fail "an orphan secret reached a new provider: $(secretly twin)"
ctl provider rm twin
kill "$kr"
wait "$kr" 2>/dev/null || true
echo "keyring and orphan secret checks passed"
repo=$(ctl remote add "http://127.0.0.1:$port/$anchor" --wait --json | jq_ "r['output']['repo_id']")
ctl stream TEST-12 160 90 >/dev/null
dawn() { ctl pkg ls --json | jq_ "len([p for p in r['packages'] if p['id'] == 'org.example.dawn'])"; }
for _ in 1 2 3; do
  item=$(ctl pkg install org.example.dawn --wait --json | jq_ "r['output']['item']")
  ctl lib apply "$item" -o TEST-12 >/dev/null 2>&1 & applying=$!
  ctl pkg uninstall org.example.dawn >/dev/null 2>&1 & removing=$!
  wait "$applying" || true
  wait "$removing" || true
  showing=$(field "'org.example.dawn' in (s['outputs']['TEST-12']['image'] or '')")
  [ "$(dawn)$showing" = 1True ] || [ "$(dawn)$showing" = 0False ] ||
    fail "uninstall and apply raced into $(dawn) packages while the output shows it: $showing"
  ctl set "$ROOT/tests/fixtures/green-64x32.png" -o TEST-12
  [ "$(dawn)" = 0 ] || ctl pkg uninstall org.example.dawn
  ctl pkg install org.example.dawn >/dev/null & installing=$!
  ctl pkg uninstall org.example.dawn >/dev/null 2>&1 & removing=$!
  wait "$installing" || true
  wait "$removing" || true
  wait_for 30 idle || fail "jobs did not finish: $(bus Daemon1 ListJobs)"
  [ "$(dawn)" = "$(ctl lib search 'kind:package dawn' --json | jq_ "r['total']")" ] ||
    fail "install and uninstall raced into $(dawn) packages and $(ctl lib search 'kind:package dawn')"
  [ "$(dawn)" = 0 ] || ctl pkg uninstall org.example.dawn
done
ctl release TEST-12 && ctl release TEST-8
ctl remote rm "${repo:0:8}"
echo "install, uninstall and apply race checks passed"
