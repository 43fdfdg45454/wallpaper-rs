#!/usr/bin/env bash
set -euo pipefail
examples=$XDG_DATA_HOME/wallpaper-rs/examples
mkdir -p "$examples"
for dir in "$ROOT"/scenes/*/; do ctl pack build "$dir" -o "$examples/$(basename "$dir").wpak" >/dev/null; done
packed=$(cat "$examples"/*.wpak | wc -c) && count=$(grep -l '"kind": "scene"' "$ROOT"/scenes/*/manifest.json | wc -l)
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
snow=$(example snowfall) && dusk=$(example dusk) && orbits=$(example orbits) && flashing=$(example pulse)
[ "$(reply "r['about']['photosensitive'], r['about']['languages'], 'audio' in r['item']['package']['facets']" Library1 GetItem s "$flashing")" = \
  "(True, ['en', 'es'], True)" ] || fail "example metadata: $(ctl lib show "$flashing")"
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
printf '%s\n' 'context.spa-libs = { audio.convert.* = audioconvert/libspa-audioconvert support.* = support/libspa-support }' \
  'context.modules = [ { name = libpipewire-module-protocol-native } { name = libpipewire-module-client-node }' \
  '{ name = libpipewire-module-adapter } { name = libpipewire-module-metadata }' \
  '{ name = libpipewire-module-protocol-pulse args = { server.address = [ "unix:native" ] } } ]' >"$WORK/pulse.conf"
pw-cli create-node adapter '{ factory.name=support.null-audio-sink node.name=wprs-scene-sink
  media.class=Audio/Sink object.linger=true audio.position=[FL FR] }' >/dev/null
pipewire -c "$WORK/pulse.conf" >"$WORK/pulse.log" 2>&1 & pulse=$!; pids+=("$pulse")
wait_for 10 test -S "$XDG_RUNTIME_DIR/pulse/native" || fail "no PulseAudio server for the default audio output"
streams() {
  pw-dump 2>/dev/null | python3 -c "import json, sys; nodes = [o['info']['props'] for o in json.load(sys.stdin) if o['type'].endswith('Node')]
print(sum(p.get('media.class') == 'Stream/Output/Audio' and p.get('application.process.binary') == 'wprsd' for p in nodes))"
}
heard() { [ "$(streams)" = "$1" ]; }
speakers() { expect "s['outputs']['TEST-15']['sound'], s['outputs']['TEST-16']['sound']" "$1"; }
pond=$(example pond) && lilies=io.github.wallpaper_rs.example.pond
read -r _ _ _ s15 < <(ctl stream TEST-15 160 90)
read -r _ _ _ s16 < <(ctl stream TEST-16 160 90)
ctl mute && ctl lib apply "$snow" -o TEST-15 2>"$WORK/apply.err" && consume "$s15"
speakers "('off', '')"
sleep 1
heard 0 || fail "a muted scene opened an audio stream"
grep -v "^TEST-15: its particles run on the processor, so fewer of them are shown$" "$WORK/apply.err" >"$WORK/hints.err" || true
if [ -s "$WORK/hints.err" ]; then fail "a scene without inputs printed a hint: $(cat "$WORK/hints.err")"; fi
expect "s['outputs']['TEST-15']['particles'] in ('cpu', 'gpu')" "True"
if grep -q "particles run on the processor" "$WORK/apply.err"; then expect "s['outputs']['TEST-15']['particles']" "cpu"; fi
ctl volume 50 && speakers "('playing', '')"
wait_for 10 heard 1 || fail "no audio stream while the scene with sound is shown"
ctl config playback.scene_sound false && speakers "('off', '')"
wait_for 10 heard 0 || fail "the scene sound went on after it was turned off for every scene"
ctl config playback.scene_sound true && speakers "('playing', '')"
wait_for 10 heard 1 || fail "the scene sound did not come back when it was allowed again"
ctl pause && speakers "('paused', '')"
heard 1 || fail "a paused scene closed its audio stream"
ctl resume && speakers "('playing', '')"
ctl prop set volume 0 -o TEST-15 && speakers "('off', '')"
wait_for 10 heard 0 || fail "a scene turned down to 0 kept its audio stream"
ctl prop reset -o TEST-15 && speakers "('playing', '')"
ctl prop set wind calm -o TEST-15
wait_for 10 heard 0 || fail "a scene whose sound layers all stopped kept its audio stream"
speakers "('off', '')"
ctl prop reset -o TEST-15 && speakers "('playing', '')"
ctl lib apply "$snow" -o TEST-16 && consume "$s16" && speakers "('playing', 'elsewhere')"
wait_for 10 heard 1 || fail "a scene on two outputs played twice: $(streams)"
ctl covered TEST-15 true && speakers "('elsewhere', 'playing')"
ctl covered TEST-15 false
heard 1 || fail "the sound did not move to the other output: $(streams)"
ctl mute && speakers "('off', 'off')"
wait_for 10 heard 0 || fail "muting kept the audio stream"
ctl unmute
wait_for 10 heard 1 || fail "unmuting did not bring the sound back"
kill "$pulse" && { wait "$pulse" 2>/dev/null || true; }
wait_for 10 heard 0 || fail "the sound stream outlived its audio output"
expect "s['outputs']['TEST-15']['error'], s['outputs']['TEST-15']['playing']" "(None, True)"
pipewire -c "$WORK/pulse.conf" >"$WORK/pulse-again.log" 2>&1 & pulse=$!; pids+=("$pulse")
wait_for 30 heard 1 || fail "the scene sound did not come back when an audio output appeared"
ctl lib apply "$dusk" -o TEST-15 && ctl lib apply "$dusk" -o TEST-16 && speakers "('', '')"
wait_for 10 heard 0 || fail "the audio stream outlived its scene"
ctl lib apply "$pond" -o TEST-15 2>"$WORK/hint.err"
grep -q "wprs-ctl input grant $lilies pointer" "$WORK/hint.err" ||
  fail "no grant command for a scene that follows the pointer: $(cat "$WORK/hint.err")"
wait_for 10 reads "r['shown']['TEST-15']['title'], r['declined'].get('$lilies')" "('Pond', None)" ||
  fail "the scene asking for the pointer was not declared: $(ctl input status --json)"
ctl input revoke "$lilies" pointer
ctl input revoke "$lilies" clicks
[ "$(inputs "r['declined'].get('$lilies')")" = "['pointer', 'clicks']" ] || fail "not now was forgotten"
ctl lib apply "$pond" -o TEST-15 2>"$WORK/hint.err"
[ ! -s "$WORK/hint.err" ] || fail "a declined scene asked again: $(cat "$WORK/hint.err")"
ctl input grant "$lilies" pointer
[ "$(inputs "r['declined'].get('$lilies')")" = "['clicks']" ] || fail "a grant kept the decline"
ctl input revoke "$lilies" pointer && ctl mute
leave && ctl release TEST-15 && ctl release TEST-16 && expect "list(s['outputs'])" "[]"
kill "$pulse" && { wait "$pulse" 2>/dev/null || true; }
echo "scene sound and consent checks passed"
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
maps=$WORK/maps && mkdir -p "$maps" && cp "$WORK/relief.png" "$maps/hills.png"
ctl edit depth import "$maps/hills.png" "$ROOT/tests/fixtures/bands-640x180.png" >/dev/null
[ -f "$maps/hills.png.depth.png" ] || fail "the depth map was not saved beside the image"
maps_source=$(ctl lib add "$maps" --wait | sed -n 1p)
indexed() { ctl lib search "source:$maps_source" --json | js "[i['title'] for i in r['items']]"; }
[ "$(indexed)" = "['hills']" ] || fail "a depth map became a wallpaper: $(indexed)"
ctl lib unsource "$maps_source"
echo "depth parallax checks passed"
