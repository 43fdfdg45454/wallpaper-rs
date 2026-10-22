#!/usr/bin/env bash
set -euo pipefail

BIN=$(realpath "${1:?usage: $0 <bin-dir>}")
ROOT=$(cd "$(dirname "$0")/.." && pwd)
WORK=$(mktemp -d /tmp/wprs-it.XXXXXX)
export XDG_RUNTIME_DIR=$WORK/run XDG_CONFIG_HOME=$WORK/config XDG_DATA_HOME=$WORK/data XDG_CACHE_HOME=$WORK/cache
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

RUST_LOG=$log "$BIN/wprsd" >"$WORK/wprsd.log" 2>&1 & daemon=$!; pids+=("$daemon")
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

pics=$WORK/pictures
mkdir -p "$pics/videos" "$pics/.hidden" "$WORK/import"
for name in harbour dunes forest; do cp "$ROOT/tests/fixtures/green-64x32.png" "$pics/$name.png"; done
cp "$ROOT/tests/fixtures/magenta-64x32.webm" "$pics/videos/clip.webm"
cp "$ROOT/tests/fixtures/green-64x32.png" "$pics/.hidden/skipped.png"
cp "$ROOT/tests/fixtures/green-64x32.png" "$WORK/import/sunset.png"
dbus-monitor --session "type='signal',interface='io.github.wallpaper_rs.Daemon1'" \
  "type='signal',interface='io.github.wallpaper_rs.Library1'" >"$WORK/library-signals.log" 2>&1 & pids+=($!)
sleep 0.5
[ "$(reply "len(r['options']), len(r['providers']), r['thumb_sizes']" Daemon1 GetSchema)" = "(11, 7, [256, 512, 1024])" ] ||
  fail "schema: $(bus Daemon1 GetSchema)"
read -r source job < <(bus Library1 AddSource sb "$pics" true | out "' '.join(map(str, d))")
wait_for 20 finished "$job" || fail "scan job: $(bus Daemon1 GetJob t "$job")"
wait_for 5 signalled JobProgress "uint64 $job" || fail "no progress for job $job"
wait_for 5 signalled JobFinished "string \"done\"" || fail "no JobFinished for job $job"
refused "overlaps the source" Library1 AddSource sb "$pics/videos" false
refused "contains wallpaper-rs data" Library1 AddSource sb "$WORK" true
[ "$(reply "r['total']" Library1 Search suu "" 0 50)" = 4 ] || fail "search: $(bus Library1 Search suu "" 0 50)"
[ "$(reply "r['total']" Library1 Search suu "kind:video" 0 0)" = 1 ] || fail "video search"
read -r dunes forest harbour < <(reply "' '.join(i['uid'] for i in r['items'])" Library1 Search suu "kind:image sort:title" 0 3)
clip=$(reply "r['items'][0]['uid']" Library1 Search suu "kind:video" 0 1)
[ "$(bus Library1 Resolve as 1 "${harbour: -8}" | out "d[0][0]")" = "$harbour" ] || fail "resolve by suffix"
refused "no item matches \"ZZZZZZZZ\"" Library1 Resolve as 1 ZZZZZZZZ
bus Library1 SetTags asasas 1 "$harbour" 1 sea 0 >/dev/null
bus Library1 SetRating asu 2 "$harbour" "$dunes" 4 >/dev/null
bus Library1 SetFavorite asb 1 "$harbour" true >/dev/null
refused "rating must be 0 to 5" Library1 SetRating asu 1 "$harbour" 6
[ "$(reply "r['tags']" Library1 ListTags)" = "[{'name': 'sea', 'count': 1}]" ] || fail "tags: $(bus Library1 ListTags)"
[ "$(reply "[i['uid'] for i in r['items']]" Library1 Search suu "tag:sea rating:4 fav:" 0 5)" = "['$harbour']" ] ||
  fail "search by tag, rating and favorite"
wait_for 30 idle || fail "analysis did not finish: $(bus Daemon1 ListJobs)"
[ "$(reply "r['item']['width'], r['item']['color'], r['item']['path'].endswith('/harbour.png')" Library1 GetItem s "$harbour")" = \
  "(64, 65280, True)" ] || fail "analysis: $(bus Library1 GetItem s "$harbour")"
[ "$(reply "r['item']['duration_ms'] > 0, r['item']['audio']" Library1 GetItem s "$clip")" = "(True, False)" ] ||
  fail "video analysis: $(bus Library1 GetItem s "$clip")"
thumb=$(bus Library1 Thumbnail su "$harbour" 256 | out "d[0]")
[ "$(head -c 3 "$thumb" | od -An -tx1 | tr -d ' ')" = ffd8ff ] || fail "the thumbnail is not a JPEG"
[ "$(stat -c %a "$thumb") $(stat -c %a "$(dirname "$thumb")")" = "600 700" ] || fail "thumbnails are not private"
[ "$(stat -c %a "$XDG_DATA_HOME/wallpaper-rs/library.db")" = 600 ] || fail "library.db is not private"
[ -s "$(bus Library1 Thumbnail su "$clip" 512 | out "d[0]")" ] || fail "no video thumbnail"
refused "size must be one of" Library1 Thumbnail su "$clip" 300
[ "$(bus Library1 RequestThumbnails asu 2 "$dunes" "$forest" 512 | out "d[0]")" = "{}" ] || fail "thumbnails were cached early"
wait_for 20 signalled ThumbnailReady "$forest" || fail "no ThumbnailReady for $forest"
wait_for 20 signalled ThumbnailReady "$dunes" || fail "no ThumbnailReady for $dunes"
forest_thumb=$(bus Library1 RequestThumbnails asu 1 "$forest" 512 | out "d[0]['$forest']")
serial=$(busctl --user --json=short get-property io.github.wallpaper_rs.Daemon /io/github/wallpaper_rs/Daemon \
  io.github.wallpaper_rs.Daemon1 Serial | out "d")
bus Library1 SetTitle ss "$dunes" "Dunes at noon" >/dev/null
bus Daemon1 WaitChanged tu "$serial" 5000 | out "d[1]" | grep -q library || fail "WaitChanged missed the library change"
refused "remove the file or the folder" Library1 Remove asb 1 "$forest" false
rm "$pics/forest.png"
job=$(bus Library1 Rescan s "" | out "d[0]")
wait_for 20 finished "$job" || fail "rescan: $(bus Daemon1 GetJob t "$job")"
[ "$(reply "r['item']['missing']" Library1 GetItem s "$forest")" = True ] || fail "the deleted file is not missing"
bus Library1 Remove asb 1 "$forest" false >/dev/null
[ ! -e "$forest_thumb" ] || fail "the thumbnail of a removed item was kept"
job=$(bus Library1 Import asb 1 "$WORK/import/sunset.png" true | out "d[0]")
wait_for 20 finished "$job" || fail "import: $(bus Daemon1 GetJob t "$job")"
sunset=$(reply "r['job']['output']['imported'][0]['uid']" Daemon1 GetJob t "$job")
copy=$XDG_DATA_HOME/wallpaper-rs/files/sunset.png
[ "$(stat -c %a "$copy")" = 600 ] || fail "the imported copy is not private"
[ "$(reply "r['items'][0]['imported'], r['items'][0]['removable'], r['items'][0]['title']" Library1 Search suu "sunset" 0 1)" = \
  "(True, True, 'sunset')" ] || fail "imported copy: $(bus Library1 Search suu sunset 0 1)"
ctl stream TEST-7 160 90 >/dev/null
bus Library1 Apply ssss "$harbour" TEST-7 fit "" >/dev/null
expect "s['outputs']['TEST-7']['item']" "$harbour"
expect "s['outputs']['TEST-7']['fit']" fit
python3 - "$WORK/import/calm.wpak" "$ROOT/tests/fixtures/green-64x32.png" <<'PACKAGE'
import json, sys, zipfile
manifest = {"schema": "wallpaper-rs.wpak/1", "id": "org.example.calm", "version": "1.0.0", "title": "Calm",
            "license": "CC0-1.0", "kind": "image", "entry": "image.png"}
with zipfile.ZipFile(sys.argv[1], "w") as package:
    package.writestr("manifest.json", json.dumps(manifest))
    package.write(sys.argv[2], "image.png")
PACKAGE
job=$(bus Library1 Import asb 1 "$WORK/import/calm.wpak" false | out "d[0]")
wait_for 20 finished "$job" || fail "package import: $(bus Daemon1 GetJob t "$job")"
calm=$(reply "r['job']['output']['imported'][0]['uid']" Daemon1 GetJob t "$job")
bus Library1 Apply ssss "$calm" TEST-7 "" "" >/dev/null
expect "s['outputs']['TEST-7']['item']" "$calm"
read -r _ _ _ serial7 < <(ctl stream TEST-7 160 90)
retry 10 sh -c "'$BIN/probe' $serial7 3 | grep -q 'center bgra \[0, 255, 0, 255\]'" || fail "the package is not shown"
[ "$(find "$XDG_CACHE_HOME/wallpaper-rs/blobs" -type f | wc -l)" = 1 ] || fail "the package entry was not materialized once"
if ctl set "$WORK/import/calm.wpak" 2>"$WORK/wpak.out"; then fail "SetWallpaper accepted a package"; fi
grep -q "import the package" "$WORK/wpak.out" || fail "package message: $(cat "$WORK/wpak.out")"
ctl release TEST-7 && expect "list(s['outputs'])" "[]"
bus Library1 Remove asb 1 "$calm" false >/dev/null
[ "$(find "$XDG_CACHE_HOME/wallpaper-rs/blobs" -type f | wc -l)" = 0 ] || fail "the package entry was kept"
bus Library1 Remove asb 1 "$sunset" true >/dev/null
[ ! -e "$copy" ] || fail "the imported copy was not deleted"
bus Library1 RemoveSource s "$source" >/dev/null
[ "$(reply "len(r['sources']), " Library1 ListSources)" = "(0,)" ] || fail "sources: $(bus Library1 ListSources)"
[ "$(reply "r['total']" Library1 Search suu "" 0 0)" = 0 ] || fail "items were kept with their source"
[ ! -e "$thumb" ] || fail "thumbnails were kept with their source"
echo "library checks passed"

js() { python3 -c "import json, sys; r = json.load(sys.stdin); print(eval(sys.argv[1]))" "$1"; }
png() {
  python3 - "$1" "$2" <<'PNG'
import struct, sys, zlib
def chunk(kind, data):
    return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))
pixels = (b"\0" + bytes.fromhex(sys.argv[2]) * 64) * 32
with open(sys.argv[1], "wb") as out:
    out.write(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", 64, 32, 8, 2, 0, 0, 0)))
    out.write(chunk(b"IDAT", zlib.compress(pixels)) + chunk(b"IEND", b""))
PNG
}
rot=$WORK/rotation
mkdir -p "$rot"
png "$rot/a-red.png" ff0000
png "$rot/b-blue.png" 0000ff
cp "$ROOT/tests/fixtures/green-64x32.png" "$rot/c-green.png"
rot_source=$(ctl lib add "$rot" --wait | sed -n 1p)
read -r red blue green < <(ctl lib search "sort:title" --json | js "' '.join(i['uid'] for i in r['items'])")
ctl playlist new Colors >/dev/null
ctl playlist add Colors "$red" "$blue" "$green"
[ "$(ctl playlist show Colors --json | js "r['total'], r['playlist']['query']")" = "(3, None)" ] ||
  fail "manual playlist: $(ctl playlist show Colors --json)"
read -r _ _ _ s8 < <(ctl stream TEST-8 160 90)
shown="s['outputs']['TEST-8']['item']"
color() { "$BIN/probe" "$s8" 3 | grep -q "center bgra \[$1\]"; }
ctl playlist start Colors -o TEST-8
expect "$shown" "$red"
expect "s['outputs']['TEST-8']['playlist']['name'], s['outputs']['TEST-8']['playlist']['count']" "('Colors', 3)"
retry 10 color "0, 0, 255, 255" || fail "red is not shown: $("$BIN/probe" "$s8" 3)"
ctl next -o TEST-8 && expect "$shown" "$blue"
retry 10 color "255, 0, 0, 255" || fail "blue is not shown: $("$BIN/probe" "$s8" 3)"
ctl prev -o TEST-8 && expect "$shown" "$red"
ctl next && ctl next && expect "$shown" "$green"
ctl next -o TEST-8 && expect "$shown" "$red"
if ctl next -o TEST-9 2>/dev/null; then fail "next on an output without playlist"; fi
ctl playlist move Colors 0 2 && expect "$shown" "$red"
ctl next -o TEST-8 && expect "$shown" "$blue"
refused "the playlist changed" Playlists1 RemoveEntries sauas Colors 1 0 1 "$red"
ctl playlist remove-at Colors 0 && expect "$shown" "$green"
[ "$(ctl playlist history -o TEST-8 --json | js "r['entries'][0]['item']['uid'], len(r['entries']) >= 6")" = "('$green', True)" ] ||
  fail "history: $(ctl playlist history -o TEST-8 --json)"
ctl playlist rule Colors interval 10s
[ "$(ctl playlist rule Colors interval)" = 10s ] || fail "rule: $(ctl playlist rule Colors)"
rotated() { [ "$(field "$shown")" = "$red" ]; }
wait_for 20 rotated || fail "the playlist did not rotate on its own: $(ctl state)"
ctl playlist rule Colors interval 0s
if ctl playlist rule Colors interval 5s 2>/dev/null; then fail "an interval below 10s was accepted"; fi
ctl stream TEST-9 160 90 >/dev/null
ctl playlist rule Colors synced true
ctl playlist start Colors
expect "s['outputs']['TEST-9']['item'] == s['outputs']['TEST-8']['item']" True
first=$(field "$shown")
ctl next -o TEST-9
expect "s['outputs']['TEST-8']['item'] == s['outputs']['TEST-9']['item'] != '$first'" True
ctl lib tag "$red" "$blue" --add warm
ctl playlist new Warm --query "tag:warm" >/dev/null
[ "$(ctl playlist show Warm --json | js "r['total'], r['playlist']['query']")" = "(2, 'tag:warm')" ] ||
  fail "smart playlist: $(ctl playlist show Warm --json)"
ctl playlist start Warm -o TEST-8
expect "$shown in ('$red', '$blue'), s['outputs']['TEST-8']['playlist']['count']" "(True, 2)"
ctl lib tag "$green" --add warm
expect "s['outputs']['TEST-8']['playlist']['count']" 3
before=$(field "$shown")
ctl next -o TEST-8 && expect "$shown != '$before'" True
kept=$(field "$shown")
ctl playlist rule Warm interval 10s
ctl lib tag "$red" "$blue" "$green" --remove warm
expect "s['outputs']['TEST-8']['playlist']['count'], $shown" "(0, '$kept')"
sleep 11
ticks() { awk '{print $14 + $15}' "/proc/$daemon/stat"; }
idle=$(ticks) && sleep 2 && idle=$(($(ticks) - idle))
[ "$idle" -lt 50 ] || fail "the daemon used $idle ticks in 2 s while an empty playlist was due"
ctl lib tag "$red" --add warm
expect "s['outputs']['TEST-8']['playlist']['count'], $shown" "(1, '$red')"
kept=$red
ctl playlist stop -o TEST-8
expect "s['outputs']['TEST-8']['playlist'], $shown" "(None, '$kept')"
ctl playlist rm Warm
[ "$(ctl playlist ls --json | js "[p['name'] for p in r['playlists']]")" = "['Colors']" ] || fail "playlists: $(ctl playlist ls)"
echo "playlist checks passed"

ctl lib apply "$red" -o TEST-8 -f center
profile=$(ctl profile save Evening)
ctl lib apply "$green" -o TEST-8 && expect "$shown" "$green"
ctl profile use Evening
expect "$shown, s['outputs']['TEST-8']['fit'], s['profile']" "('$red', 'center', '$profile')"
[ "$(ctl profile save evening)" = "$profile" ] || fail "saving a profile again changed its uid"
[ "$(ctl profile ls --json | js "[(p['name'], p['active'], p['outputs']['TEST-8']['item']) for p in r['profiles']]")" = \
  "[('Evening', True, '$red')]" ] || fail "profiles: $(ctl profile ls --json)"
ctl profile off && expect "s['profile']" None
ctl profile auto Evening outputs TEST-8
[ "$(ctl profile auto Evening outputs)" = TEST-8 ] || fail "auto rule: $(ctl profile auto Evening)"
ctl lib apply "$green" -o TEST-8 && expect "$shown" "$green"
ctl release TEST-9
activated() { [ "$(field "s['profile'], $shown")" = "('$profile', '$red')" ]; }
wait_for 10 activated || fail "the profile was not activated for its outputs: $(ctl state)"
preset=$(ctl preset save "$red" Dim fit=tile background=#102030)
ctl lib apply "$red" -o TEST-8 -p Dim
expect "s['outputs']['TEST-8']['fit'], s['outputs']['TEST-8']['background'], s['outputs']['TEST-8']['preset']" \
  "('tile', '#102030', '$preset')"
[ "$(ctl preset save "$red" dim fit=fit)" = "$preset" ] || fail "saving a preset again changed its uid"
expect "s['outputs']['TEST-8']['fit'], s['outputs']['TEST-8']['background']" "('fit', '#102030')"
ctl preset rm "$red:Dim"
expect "s['outputs']['TEST-8']['preset'], s['outputs']['TEST-8']['fit']" "(None, 'fit')"
ctl preset save - Wide fit=fill >/dev/null
[ "$(ctl preset ls --json | js "[(p['name'], p['item'], p['value']['fit']) for p in r['presets']]")" = "[('Wide', None, 'fill')]" ] ||
  fail "global presets: $(ctl preset ls --json)"
ctl release TEST-8 && expect "list(s['outputs'])" "[]"
ctl lib unsource "$rot_source"
echo "profile and preset checks passed"

restored() {
  read -r _ _ _ serial < <(ctl stream TEST-1 320 180) || return 1
  "$BIN/probe" "$serial" 3 | grep -q "frame 320x180 .* buffers 4 center bgra \[0, 255, 0, 255\]"
}
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
