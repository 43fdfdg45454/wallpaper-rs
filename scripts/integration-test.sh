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
printf '[playback\nvolume = 0.5\n[models]\nallow_download = false\n' >"$broken"
RUST_LOG=$log "$BIN/wprsd" --config "$broken" >"$WORK/broken.log" 2>&1 & broken_pid=$!; pids+=("$broken_pid")
wait_for 30 ctl state
expect "s['config_error'] is not None" True
if ctl reload 2>/dev/null; then fail "reload accepted a broken configuration"; fi
expect "s['config_error'] is not None" True
expect "s['options']['models.allow_download']" False
ctl config pause.on_battery false
expect "s['config_error']" None
grep -qx '\[playback' "$broken.broken" || fail "the broken configuration was not kept"
grep -q 'on_battery = false' "$broken" || fail "the new configuration was not saved"
grep -qx 'allow_download = false' "$broken" || fail "a broken configuration turned downloads back on"
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
[ "$(reply "len(r['options']), len(r['providers']), r['thumb_sizes']" Daemon1 GetSchema)" = "(14, 7, [256, 512, 1024])" ] ||
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
morning=$(ctl profile save Morning)
ctl profile use Morning && expect "s['profile']" "$morning"
[ "$(stat -c %a "$XDG_DATA_HOME/wallpaper-rs/profile-hold.json")" = 600 ] || fail "the manual profile choice is not private"
ctl release TEST-8 && expect "list(s['outputs'])" "[]"
ctl lib unsource "$rot_source"
echo "profile and preset checks passed"
jq_() { python3 -c "import json, sys; r = json.load(sys.stdin); print(eval(sys.argv[1]))" "$1"; }
config=$XDG_CONFIG_HOME/wallpaper-rs/config.toml
daily=$(ctl provider add daily bing refresh=0s count=2)
[ "$(ctl provider ls --json | jq_ "[(p['id'], p['state'], p['hosts'], p['next_sync'], p['source']) for p in r['providers']]")" = \
  "[('daily', 'idle', ['www.bing.com'], None, '$daily')]" ] || fail "providers: $(ctl provider ls --json)"
ctl provider set daily count 3
grep -q '^\[providers.daily\]' "$config" || fail "provider config: $(cat "$config")"
grep -q '^count = 3' "$config" || fail "provider option: $(cat "$config")"
[ "$(ctl lib sources --json | jq_ "[s['provider'] for s in r['sources']]")" = "['daily']" ] || fail "provider source"
refused "belongs to provider" Library1 RemoveSource s "$daily"
if ctl provider set daily count 9 2>"$WORK/provider.out"; then fail "an invalid provider option was accepted"; fi
if ctl provider add daily bing 2>/dev/null; then fail "a provider id was added twice"; fi
if printf 'hunter2\n' | ctl provider add photos webdav url=https://dav.example.org/files user=me 2>"$WORK/keyring.out"; then
  fail "a secret was accepted without a keyring"
fi
grep -q "no Secret Service is available" "$WORK/keyring.out" || fail "keyring message: $(cat "$WORK/keyring.out")"
grep -q photos "$config" && fail "a provider was created although its secret could not be stored"
ctl provider add photos webdav url=https://dav.example.org/files user=me --no-secret >/dev/null
[ "$(ctl provider ls --json | jq_ "[p['state'] for p in r['providers'] if p['id'] == 'photos']")" = "['needs_secret']" ] ||
  fail "provider without secret: $(ctl provider ls --json)"
if ctl provider sync photos 2>"$WORK/sync.out"; then fail "a provider synced without its secret"; fi
grep -q "needs its secret" "$WORK/sync.out" || fail "sync message: $(cat "$WORK/sync.out")"
if printf 'hunter2\n' | ctl provider secret photos 2>"$WORK/secret.out"; then fail "a secret was stored without a keyring"; fi
grep -q "no Secret Service is available" "$WORK/secret.out" || fail "secret message: $(cat "$WORK/secret.out")"
if ctl provider set photos url http://dav.example.org/files 2>"$WORK/plain.out"; then fail "plain http was accepted"; fi
grep -q "plain http" "$WORK/plain.out" || fail "plain http message: $(cat "$WORK/plain.out")"
if ctl provider add leak webdav url=https://me:pw@dav.example.org user=me --no-secret 2>"$WORK/url.out"; then
  fail "credentials in a URL were accepted"
fi
grep -q "not in the URL" "$WORK/url.out" || fail "URL message: $(cat "$WORK/url.out")"
grep -q hunter2 "$config" "$WORK"/*.log && fail "a secret reached the configuration or a log"
ctl provider rm photos && ctl provider rm daily --purge
[ "$(ctl provider ls --json | jq_ "len(r['providers'])")" = 0 ] || fail "providers were kept"
[ ! -e "$XDG_CACHE_HOME/wallpaper-rs/providers/daily" ] || fail "the provider cache was kept"
echo "provider checks passed"

site=$WORK/site
mkdir -p "$site/src"
cp "$ROOT/tests/fixtures/green-64x32.png" "$site/src/image.png"
cp "$ROOT/tests/fixtures/green-64x32.png" "$site/src/preview.png"
cat >"$site/src/manifest.json" <<'MANIFEST'
{"schema": "wallpaper-rs.wpak/1", "id": "org.example.dawn", "version": "1.0.0", "title": "Dawn",
 "license": "CC0-1.0", "kind": "image", "entry": "image.png", "preview": "preview.png", "tags": ["sky"]}
MANIFEST
(
  cd "$site"
  ctl repo keygen root --no-password >/dev/null
  ctl repo keygen online --no-password >/dev/null
  ctl pack build src -o dawn.wpak >/dev/null
  ctl pack build "$ROOT/scenes/dusk" -o dusk.wpak >/dev/null
  ctl repo init repo --name Example --root-key root.pub --online-key online.pub >/dev/null 2>&1
  ctl repo sign-root repo --key root.key >/dev/null
  ctl repo add repo dawn.wpak >/dev/null
  ctl repo add repo dusk.wpak >/dev/null
  ctl repo publish repo --key online.key >/dev/null
)
anchor=$(ctl repo status "$site/repo" | sed -n 's/^anchor = //p')
port=$(python3 -c "import socket; s = socket.socket(); s.bind(('127.0.0.1', 0)); print(s.getsockname()[1])")
python3 -m http.server --bind 127.0.0.1 --directory "$site/repo" "$port" >"$WORK/http.log" 2>&1 & pids+=($!)
wait_for 10 python3 -c "import urllib.request; urllib.request.urlopen('http://127.0.0.1:$port/entry/latest')" ||
  fail "the repository server did not start"
if ctl remote add "http://127.0.0.1:$port/" 2>"$WORK/anchor.out"; then fail "a repository without anchor was added"; fi
grep -q "fingerprint or repo_id" "$WORK/anchor.out" || fail "anchor message: $(cat "$WORK/anchor.out")"
repo=$(ctl remote add "http://127.0.0.1:$port/$anchor" --wait --json | jq_ "r['output']['repo_id']")
[ "$(ctl remote ls --json | jq_ "[(x['name'], x['hosts'], x['packages'], x['stale']) for x in r['repos']]")" = \
  "[('Example', ['127.0.0.1'], 2, False)]" ] || fail "repositories: $(ctl remote ls --json)"
[ "$(ctl pkg search sky --json | jq_ "[(p['id'], p['version'], p['preview'], p['repo_name']) for p in r['packages']]")" = \
  "[('org.example.dawn', '1.0.0', True, 'Example')]" ] || fail "browse: $(ctl pkg search sky --json)"
preview=$(ctl pkg preview "${repo:0:8}/org.example.dawn")
[ "$(head -c 3 "$preview" | od -An -tx1 | tr -d ' ')" = ffd8ff ] || fail "the preview is not a re-encoded JPEG"
[ "$(stat -c %a "$preview") $(stat -c %a "$(dirname "$preview")")" = "600 700" ] || fail "previews are not private"
[ "$(ctl pkg preview org.example.dawn)" = "$preview" ] || fail "the cached preview was not reused"
ctl remote refresh --wait >/dev/null
item=$(ctl pkg install org.example.dawn --wait --json | jq_ "r['output']['item']")
[ "$(ctl pkg ls --json | jq_ "[(p['id'], p['version'], p['item']) for p in r['packages']]")" = \
  "[('org.example.dawn', '1.0.0', '$item')]" ] || fail "installed: $(ctl pkg ls --json)"
[ "$(ctl lib show "$item" --json | jq_ "r['installed']['version'], r['item']['title'], 'sky' in r['item']['tags']")" = \
  "('1.0.0', 'Dawn', True)" ] || fail "installed item: $(ctl lib show "$item" --json)"
refused "use Repos1.Uninstall" Library1 Remove asb 1 "$item" false
ctl pkg uninstall org.example.dawn
[ "$(ctl pkg ls --json | jq_ "len(r['packages'])")" = 0 ] || fail "the package was kept"
[ "$(ctl lib search kind:package --json | jq_ "r['total']")" = 0 ] || fail "the package item was kept"
dusk=io.github.wallpaper_rs.example.dusk
[ "$(ctl pkg search dusk --json | jq_ "[(p['id'], p['kind']) for p in r['packages']]")" = "[('$dusk', 'scene')]" ] ||
  fail "browse scenes: $(ctl pkg search dusk --json)"
item=$(ctl pkg install "$dusk" --wait --json | jq_ "r['output']['item']")
[ "$(ctl lib search kind:scene --json | jq_ "[i['uid'] for i in r['items']]")" = "['$item']" ] ||
  fail "installed scene: $(ctl lib search kind:scene --json)"
ctl pkg uninstall "$dusk"
python3 -c "import socket, time; s = socket.socket(); s.bind(('127.0.0.1', 0)); s.listen(8)
print(s.getsockname()[1], flush=True); time.sleep(600)" >"$WORK/stall.port" & pids+=($!)
wait_for 5 test -s "$WORK/stall.port" || fail "the stalled server did not start"
stalled=$(bus Repos1 Add s "http://127.0.0.1:$(cat "$WORK/stall.port")/?fingerprint=$(printf '%064d' 0)" | out "d[0]")
running() { [ "$(reply "r['job']['state']" Daemon1 GetJob t "$1")" = running ]; }
wait_for 10 running "$stalled" || fail "the stalled repository job: $(bus Daemon1 GetJob t "$stalled")"
SECONDS=0
ctl remote rm "${repo:0:8}"
[ "$SECONDS" -lt 5 ] || fail "removing a repository waited for the job of another repository"
bus Daemon1 CancelJob t "$stalled" >/dev/null
[ "$(ctl remote ls --json | jq_ "len(r['repos'])")" = 0 ] || fail "the repository was kept"
[ ! -e "$preview" ] || fail "the preview was kept with its repository"
echo "repository checks passed"
looks() {
  read -r b g r _ < <("$BIN/probe" "$1" 3 | sed 's/.*center bgra \[\(.*\)\]/\1/; s/,/ /g')
  near "$r" "$2" && near "$g" "$3" && near "$b" "$4"
}
shown() { wait_for 10 looks "$@" || fail "stream $1 shows $("$BIN/probe" "$1" 3), expected $2 $3 $4"; }
pic=$WORK/edits/meadow.png
mkdir -p "$WORK/edits"
cp "$ROOT/tests/fixtures/green-64x32.png" "$pic"
ctl edit set "$pic" ev=-1
[ "$(stat -c %a "$pic.wprs.json")" = 600 ] || fail "the edit file is not private"
read -r _ _ _ edited < <(ctl stream TEST-8 160 90)
ctl set "$pic" -o TEST-8 --fit stretch
shown "$edited" 0 188 0
expect "s['outputs']['TEST-8']['edited']" True
ctl edit set "$pic" ev=-2
shown "$edited" 0 137 0
ctl edit preview "$pic" ev=0
shown "$edited" 0 255 0
ctl edit preview "$pic" --stop
shown "$edited" 0 137 0
ctl edit overlay "$pic" '#ff0000'
shown "$edited" 255 0 0
ctl edit reset "$pic"
shown "$edited" 0 255 0
expect "s['outputs']['TEST-8']['edited']" False
if [ "$(id -u)" != 0 ]; then
  chmod 555 "$WORK/edits"
  ctl edit set "$pic" ev=-1
  if [ -e "$pic.wprs.json" ] || ! compgen -G "$XDG_DATA_HOME/wallpaper-rs/edits/*.json" >/dev/null; then
    fail "the edit of an image in a read-only folder is not in the data folder"
  fi
  shown "$edited" 0 188 0
  chmod 755 "$WORK/edits"
fi
ctl release TEST-8 && expect "list(s['outputs'])" "[]"
cp "$ROOT/tests/fixtures/bands-640x180.png" "$WORK/bands.png"
read -r _ _ _ left < <(ctl stream TEST-9 160 90)
read -r _ _ _ right < <(ctl stream TEST-10 160 90)
ctl screen TEST-9 --pos 0,0 --size 160x90 --density 4 --identity "Maker Model 9"
ctl screen TEST-10 --pos 160,0 --size 160x90 --density 4
ctl set "$WORK/bands.png" --fit fill
shown "$left" 0 255 0 && shown "$right" 0 255 0
ctl fit span
shown "$left" 255 0 0 && shown "$right" 0 0 255
expect "s['outputs']['TEST-9']['screen']['identity']" "Maker Model 9"
ctl edit span "$WORK/bands.png" --zoom 2
shown "$left" 0 255 0 && shown "$right" 0 255 0
ctl edit crop "$WORK/bands.png" 0.7,0,0.3,1 --output TEST-10
shown "$right" 0 0 255
ctl edit reset "$WORK/bands.png"
shown "$left" 255 0 0 && shown "$right" 0 0 255
ctl release TEST-9
shown "$right" 0 255 0
ctl release TEST-10 && expect "list(s['outputs'])" "[]"
echo "edit and span checks passed"

kill "$daemon"
wait "$daemon" 2>/dev/null || true
RUST_LOG=$log "$BIN/wprsd" >"$WORK/restarted.log" 2>&1 & daemon=$!; pids+=("$daemon")
wait_for 30 ctl state || fail "the daemon did not start again"
ctl stream TEST-8 160 90 >/dev/null
sleep 4
expect "s['profile']" "$morning"
ctl stream TEST-9 160 90 >/dev/null
sleep 3
ctl release TEST-9
evening() { [ "$(field "s['profile']")" = "$profile" ]; }
wait_for 10 evening || fail "the manual profile choice outlived a change of outputs: $(ctl state)"
ctl release TEST-8 && expect "list(s['outputs'])" "[]"
echo "manual profile choice kept across a restart"
scene=$WORK/scene
mkdir -p "$scene"
python3 - "$scene/glow.wpak" "$ROOT/tests/fixtures/green-64x32.png" <<'PACKAGE'
import json, sys, zipfile
manifest = {"schema": "wallpaper-rs.wpak/1", "id": "org.example.glow", "version": "1.0.0", "title": "Glow",
            "license": "CC0-1.0", "kind": "scene", "entry": "scene.json", "preview": "preview.png",
            "properties": {"tint": {"type": "color", "label": "Tint", "default": "#0000ff"}},
            "presets": [{"id": "warm", "values": {"tint": "#ff0000"}}]}
scene = {"schema": "wallpaper-rs.scene/1", "canvas": {"size": [64, 32], "fit": "stretch"},
         "assets": {"backdrop": {"path": "assets/backdrop.png"}},
         "layers": [{"id": "backdrop", "type": "image", "image": "backdrop"},
                    {"id": "tint", "type": "solid", "color": {"prop": "tint"}, "size": [32, 16]}]}
with zipfile.ZipFile(sys.argv[1], "w") as package:
    package.writestr("manifest.json", json.dumps(manifest))
    package.writestr("scene.json", json.dumps(scene))
    package.write(sys.argv[2], "preview.png")
    package.write(sys.argv[2], "assets/backdrop.png")
PACKAGE
[ "$(ctl scene bench "$scene/glow.wpak" --frames 20 --size 160x90 --json | js "r['needed'], r['created_after_warmup']")" = "(1, 0)" ] ||
  fail "scene bench: $(ctl scene bench "$scene/glow.wpak" --frames 20 --size 160x90)"
read -r _ _ _ s11 < <(ctl stream TEST-11 160 90)
job=$(bus Library1 Import asb 1 "$scene/glow.wpak" false | out "d[0]")
wait_for 20 finished "$job" || fail "scene import: $(bus Daemon1 GetJob t "$job")"
glow=$(reply "r['job']['output']['imported'][0]['uid']" Daemon1 GetJob t "$job")
[ -s "$(bus Library1 Thumbnail su "$glow" 256 | out "d[0]")" ] || fail "no thumbnail from the scene preview"
ctl lib apply "$glow" -o TEST-11
expect "s['outputs']['TEST-11']['kind'], s['outputs']['TEST-11']['item']" "('scene', '$glow')"
shown "$s11" 0 0 255
ctl prop set tint '#ff0000' -o TEST-11
shown "$s11" 255 0 0
[ "$(ctl prop get tint -o TEST-11)" = '"#ff0000"' ] || fail "property value: $(ctl prop ls -o TEST-11)"
[ "$(ctl prop ls -o TEST-11 --json | js "r['package'], list(r['properties']), r['presets'][0]['id']")" = \
  "('org.example.glow', ['tint'], 'warm')" ] || fail "properties: $(ctl prop ls -o TEST-11 --json)"
grep -q '^\[outputs.TEST-11.scenes."org.example.glow"\]' "$config" || fail "scene values: $(cat "$config")"
if ctl prop set tint red -o TEST-11 2>/dev/null; then fail "an invalid color was accepted"; fi
ctl prop reset -o TEST-11
shown "$s11" 0 0 255
ctl prop preset warm -o TEST-11
shown "$s11" 255 0 0
sleep 1
idle=$(ticks) && sleep 2 && idle=$(($(ticks) - idle))
[ "$idle" -lt 50 ] || fail "the daemon used $idle ticks in 2 s while a still scene was shown"
python3 - "$scene/clip.wpak" "$ROOT/tests/fixtures/magenta-64x32.webm" "$ROOT/tests/fixtures/green-64x32.png" <<'PACKAGE'
import json, sys, zipfile
manifest = {"schema": "wallpaper-rs.wpak/1", "id": "org.example.clip", "version": "1.0.0", "title": "Clip",
            "license": "CC0-1.0", "kind": "scene", "entry": "scene.json", "preview": "preview.png"}
scene = {"schema": "wallpaper-rs.scene/1", "canvas": {"size": [64, 32], "fit": "stretch"},
         "assets": {"clip": {"path": "assets/clip.webm"}},
         "layers": [{"id": "clip", "type": "video", "video": "clip", "size": [64, 32]}]}
with zipfile.ZipFile(sys.argv[1], "w") as package:
    package.writestr("manifest.json", json.dumps(manifest))
    package.writestr("scene.json", json.dumps(scene))
    package.write(sys.argv[2], "assets/clip.webm", zipfile.ZIP_STORED)
    package.write(sys.argv[3], "preview.png")
PACKAGE
job=$(bus Library1 Import asb 1 "$scene/clip.wpak" false | out "d[0]")
wait_for 20 finished "$job" || fail "video scene import: $(bus Daemon1 GetJob t "$job")"
clip=$(reply "r['job']['output']['imported'][0]['uid']" Daemon1 GetJob t "$job")
ctl lib apply "$clip" -o TEST-11
shown "$s11" 208 32 96
ctl release TEST-11 && expect "list(s['outputs'])" "[]"
bus Library1 Remove asb 2 "$glow" "$clip" false >/dev/null
echo "scene checks passed"
sea=org.example.sea
inputs() { ctl input status --json | python3 -c "import json, sys; r = json.load(sys.stdin); print(eval(sys.argv[1]))" "$1"; }
[ "$(inputs "r['grants'], r['capturing'], r['tracking']")" = "({}, False, False)" ] || fail "inputs start on"
ctl input grant "$sea" audio && ctl input grant "$sea" pointer && ctl input grant "$sea" audio
[ "$(inputs "r['grants']")" = "{'$sea': ['audio', 'pointer']}" ] || fail "grants: $(ctl input status --json)"
[ "$(stat -c %a "$XDG_DATA_HOME/wallpaper-rs/inputs.json")" = 600 ] || fail "inputs.json is not private"
[ "$(inputs "r['capturing'], r['tracking']")" = "(False, False)" ] || fail "a grant alone started an input"
ctl input pointer 10 10 && ctl input pointer -10 -10
ctl input status | grep -qx "audio capture: off" || fail "status: $(ctl input status)"
refused "is not audio or pointer" Inputs1 Grant ss "$sea" camera
refused "is not a scene package id" Inputs1 Grant ss "Not A Scene" audio
refused "Signature mismatch" Inputs1 SetPointer dd 1 1
ctl input revoke "$sea" audio
[ "$(inputs "r['grants']")" = "{'$sea': ['pointer']}" ] || fail "revoke: $(ctl input status --json)"
ctl input revoke "$sea" pointer && ctl input revoke "$sea" pointer
[ "$(inputs "r['grants'], r['capturing'], r['tracking']")" = "({}, False, False)" ] || fail "inputs after revoking"
grep -q "$sea" "$XDG_DATA_HOME/wallpaper-rs/inputs.json" && fail "a revoked grant was kept"
echo "input permission checks passed"
python3 - "$scene" "$ROOT/tests/fixtures/green-64x32.png" <<'PACKAGE'
import json, sys, zipfile
def package(name, scene, files):
    manifest = {"schema": "wallpaper-rs.wpak/1", "id": f"org.example.{name}", "version": "1.0.0", "title": name,
                "license": "CC0-1.0", "kind": "scene", "entry": "scene.json", "preview": "preview.png"}
    scene = {"schema": "wallpaper-rs.scene/1", "canvas": {"size": [64, 32], "fit": "stretch"}, **scene}
    with zipfile.ZipFile(f"{sys.argv[1]}/{name}.wpak", "w") as out:
        out.writestr("manifest.json", json.dumps(manifest))
        out.writestr("scene.json", json.dumps(scene))
        out.write(sys.argv[2], "preview.png")
        for path, text in files.items():
            out.writestr(path, text)
package("sea", {"layers": [{"id": "bars", "type": "spectrum", "size": [64, 32], "bands": 16}]}, {})
tint = "import { scene } from 'wprs';\nexport function update() { scene.layer('tint').color = [1, 0, 0, 1]; }\n"
package("spin", {"layers": [{"id": "tint", "type": "solid", "color": "#0000ff"}],
                 "scripts": [{"path": "scripts/tint.js"}]}, {"scripts/tint.js": tint})
PACKAGE
imported() {
  local job
  job=$(bus Library1 Import asb 1 "$1" false | out "d[0]")
  wait_for 20 finished "$job" || fail "import of $1: $(bus Daemon1 GetJob t "$job")"
  reply "r['job']['output']['imported'][0]['uid']" Daemon1 GetJob t "$job"
}
sea_uid=$(imported "$scene/sea.wpak") && spin_uid=$(imported "$scene/spin.wpak")
analysing() { pw-dump 2>/dev/null | grep -q '"media.name": "wallpaper-rs audio analysis"'; }
reads() { [ "$(inputs "$1")" = "$2" ]; }
read -r _ _ _ s12 < <(ctl stream TEST-12 160 90)
ctl lib apply "$sea_uid" -o TEST-12
wait_for 10 reads "r['shown']['TEST-12']['audio'], r['shown']['TEST-12']['scene']" "(True, '$sea')" ||
  fail "the audio scene was not declared: $(ctl input status --json)"
sleep 1
[ "$(inputs "r['capturing']")" = False ] || fail "audio was captured without permission"
if analysing; then fail "an audio stream exists without permission"; fi
ctl input grant "$sea" audio
wait_for 10 reads "r['capturing']" True || fail "no audio capture with the permission: $(ctl input status --json)"
wait_for 10 analysing || fail "no PipeWire stream for the audio analysis"
ctl lib apply "$spin_uid" -o TEST-12
wait_for 10 reads "r['capturing'], list(r['shown'])" "(False, [])" || fail "the capture outlived its scene"
wait_for 10 sh -c "! pw-dump 2>/dev/null | grep -q 'wallpaper-rs audio analysis'" || fail "the audio stream was kept"
shown "$s12" 255 0 0
expect "s['outputs']['TEST-12']['kind'], s['outputs']['TEST-12']['error']" "('scene', None)"
consume "$s12" && sleep 2
idle=$(ticks) && sleep 2 && idle=$(($(ticks) - idle)) && leave
[ "$idle" -lt 8 ] || fail "the daemon used $idle ticks in 2 s while a watched scripted scene stayed the same"
ctl release TEST-12 && expect "list(s['outputs'])" "[]"
ctl input revoke "$sea" audio
bus Library1 Remove asb 2 "$sea_uid" "$spin_uid" false >/dev/null
echo "audio and script scene checks passed"
examples=$XDG_DATA_HOME/wallpaper-rs/examples
mkdir -p "$examples"
for dir in "$ROOT"/scenes/*/; do ctl pack build "$dir" -o "$examples/$(basename "$dir").wpak" >/dev/null; done
packed=$(cat "$examples"/*.wpak | wc -c) && count=$(find "$examples" -name '*.wpak' | wc -l)
((packed < 409600 && count >= 5)) || fail "$count example scenes take $packed bytes"
rescanned() {
  local job
  job=$(bus Library1 Rescan s "" | out "d[0]")
  wait_for 60 finished "$job" || fail "rescan: $(bus Daemon1 GetJob t "$job")"
}
listed() { [ "$(reply "r['total'], {i['package']['kind'] for i in r['items']}" Library1 Search suu "kind:scene" 0 50)" = "$1" ]; }
rescanned
wait_for 10 listed "($count, {'scene'})" || fail "examples in the library: $(ctl lib search kind:scene)"
example() { reply "r['items'][0]['uid']" Library1 Search suu "kind:scene $1" 0 1; }
snow=$(example snowfall) && dusk=$(example dusk) && orbits=$(example orbits)
[ -s "$(bus Library1 Thumbnail su "$snow" 256 | out "d[0]")" ] || fail "no thumbnail from an example preview"
read -r _ _ _ s13 < <(ctl stream TEST-13 160 90)
digest() { "$BIN/probe" "$s13" 3 | sed -n 's/.* hash \([0-9a-f]*\)$/\1/p'; }
changes() { local a b; a=$(digest) && sleep 0.5 && b=$(digest) && [ -n "$a" ] && [ "$a" != "$b" ]; }
consume "$s13"
for uid in "$snow" "$orbits"; do
  ctl lib apply "$uid" -o TEST-13
  expect "s['outputs']['TEST-13']['item'], s['outputs']['TEST-13']['error']" "('$uid', None)"
  expect "s['outputs']['TEST-13']['playing']" True
  retry 10 changes || fail "example $uid stands still: $("$BIN/probe" "$s13" 3)"
done
ctl lib apply "$dusk" -o TEST-13
still() { [ "$(field "s['outputs']['TEST-13']['item'], s['outputs']['TEST-13']['playing']")" = "('$dusk', False)" ]; }
wait_for 15 still || fail "the still example keeps animating: $(ctl state)"
idle=$(ticks) && sleep 2 && idle=$(($(ticks) - idle))
[ "$idle" -lt 20 ] || fail "the still example used $idle ticks in 2 s"
leave
first=$(digest)
[ -n "$first" ] || fail "no frame from the still example: $("$BIN/probe" "$s13" 3 2>&1)"
[ "$first" = "$(digest)" ] || fail "the still example changed while nothing moved"
ctl release TEST-13 && expect "list(s['outputs'])" "[]"
rm "$examples/pulse.wpak" && rescanned
wait_for 10 listed "($((count - 1)), {'scene'})" || fail "an uninstalled example stayed: $(ctl lib search kind:scene)"
mapfile -t uids < <(reply "'\\n'.join(i['uid'] for i in r['items'])" Library1 Search suu "kind:scene" 0 50)
bus Library1 Remove asb "${#uids[@]}" "${uids[@]}" false >/dev/null && rescanned
listed "(0, set())" || fail "removed examples came back: $(ctl lib search kind:scene)"
rm -r "$examples"
echo "example scene checks passed"

read -r _ _ _ left < <(ctl stream TEST-9 160 90)
read -r _ _ _ right < <(ctl stream TEST-10 160 90)
ctl screen TEST-9 --pos 0,0 --size 160x90 --density 4
ctl screen TEST-10 --pos 160,0 --size 160x90 --density 4
ctl set "$WORK/bands.png" --fit span && ctl edit span "$WORK/bands.png" --zoom 2
shown "$left" 0 255 0 && shown "$right" 0 255 0
ctl config layout.bezel_mm 40 && expect "s['options']['layout.bezel_mm']" 40.0
shown "$left" 255 0 0 && shown "$right" 0 0 255
ctl release TEST-9 && ctl release TEST-10 && expect "list(s['outputs'])" "[]"
echo "screen frame checks passed"

cp "$ROOT/tests/fixtures/bands-640x180.png" "$WORK/relief.png"
ctl edit depth import "$WORK/relief.png" "$ROOT/tests/fixtures/bands-640x180.png" >/dev/null
ctl edit parallax "$WORK/relief.png" --source auto --steps 8
read -r _ _ _ deep < <(ctl stream TEST-11 160 90)
ctl set "$WORK/relief.png" -o TEST-11 --fit fill
case $(field "s['adapter']") in
  llvmpipe*) expect "s['outputs']['TEST-11']['parallax']" False ;;
esac
ctl config playback.software_parallax true
expect "s['outputs']['TEST-11']['parallax']" True
consume "$deep"
frames() { field "s['outputs']['TEST-11']['frames']"; }
past() { [ "$(frames)" -gt "$1" ]; }
moves() { wait_for "$1" past "$(frames)"; }
quiet() { local from; from=$(frames); sleep 1; [ "$(frames)" = "$from" ]; }
moves 10 || fail "the parallax does not move: $(ctl state)"
ctl pause
wait_for 10 quiet || fail "the parallax moves while paused"
ctl resume
moves 10 || fail "the parallax did not resume"
ctl screen TEST-11 --pos 0,0 --size 160x90 --density 4
ctl edit parallax "$WORK/relief.png" --source pointer --strength 0.03
wait_for 10 reads "r['tracking'], r['parallax']" "(True, ['TEST-11'])" ||
  fail "the pointer parallax does not follow the pointer: $(ctl input status --json)"
wait_for 10 quiet || fail "the parallax moves without a pointer"
held=$(frames)
ctl input pointer 150 80
wait_for 10 past "$held" || fail "the parallax does not follow the pointer"
wait_for 10 quiet || fail "the parallax does not settle after the pointer"
ctl edit parallax "$WORK/relief.png" --off
expect "s['outputs']['TEST-11']['parallax']" False
wait_for 10 reads "r['tracking'], r['parallax']" "(False, [])" || fail "the pointer is still followed"
wait_for 10 quiet || fail "the parallax moves after it was turned off"
cp "$ROOT/tests/fixtures/bands-640x180.png" "$WORK/torn.png" && printf 'not a png' >"$WORK/torn.png.depth.png"
ctl edit parallax "$WORK/torn.png" --source auto && ctl set "$WORK/torn.png" -o TEST-11 --fit fill
wait_for 10 grep -q "cannot use a depth map" "$WORK/restarted.log" || fail "a broken depth map was not reported"
above() { sed 's/\x1b\[[0-9;]*m//g' "$WORK"/*.log | grep -E '^[^ ]+ +(INFO|WARN|ERROR) '; }
above | grep "cannot use a depth map" >/dev/null || fail "the log level filter does not match"
if above | grep torn; then fail "a log above debug names an image"; fi
leave
ctl release TEST-11 && expect "list(s['outputs'])" "[]"
ctl config playback.software_parallax false
echo "depth parallax checks passed"

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
