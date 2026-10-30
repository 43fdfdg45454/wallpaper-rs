#!/usr/bin/env bash
set -euo pipefail
declare log
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
result=$("$BIN/probe" "$serial" 10 --dmabuf)
grep -q "type \(MemFd\|DmaBuf\) .*center bgra \[0, 255, 0, 255\]" <<<"$result" || fail "DMA-BUF consumer: $result"
case $(field "s['adapter']") in
  llvmpipe*) grep -q "type MemFd" <<<"$result" || fail "software rendering exported GPU memory: $result" ;;
  *"(Vulkan)") grep -q "type DmaBuf modifier 0x0 .*center bgra \[0, 255, 0, 255\]" <<<"$result" ||
    fail "the GPU sent no linear frame in GPU memory: $result" ;;
esac

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

result=$("$BIN/probe" "$serial2" 5 --late) || fail "a consumer that missed the first frame waited: $result"
consume "$serial2"
sleep 4
result=$("$BIN/probe" "$serial2" 5) || fail "a second consumer of a still picture waited: $result"
result=$("$BIN/probe" "$serial2" 5 --late) || fail "a second consumer that missed the first frame waited: $result"
leave
echo "late consumer checks passed"

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
expect "${output}['decode']" copy

ctl pause && expect "${output}['playing']" False && expect "s['paused']" True
ctl resume && expect "${output}['playing']" True
ctl covered TEST-1 true && expect "${output}['active']" False
ctl config pause.when_covered false && expect "${output}['active']" True
ctl config pause.when_covered true && ctl covered TEST-1 false
if ctl config playback.max_fps 241 2>/dev/null; then echo "FAIL: invalid setting accepted"; exit 1; fi
expect "s['options']['playback.max_fps']" display
expect "(${output}['refresh'], 0 < ${output}['fps'] <= 60)" "(0, True)"
ctl config playback.max_fps 20 && expect "${output}['fps']" 20
ctl config playback.max_fps display && expect "s['options']['playback.max_fps']" display
ctl volume 30 && expect "s['volume']" 0.3 && expect "s['muted']" False
ctl mute && expect "s['muted']" True
leave && expect "${output}['playing']" False
ctl release TEST-1 && expect "list(s['outputs'])" "[]"
echo "video, pause, settings and release checks passed"

sound=$WORK/sound.webm
gst-launch-1.0 -q videotestsrc num-buffers=100 ! video/x-raw,width=64,height=32,framerate=5/1 ! vp8enc ! \
  webmmux name=mux ! filesink location="$sound" audiotestsrc num-buffers=600 samplesperbuffer=1600 volume=0.5 ! \
  audio/x-raw,rate=48000 ! audioconvert ! vorbisenc ! mux.
level() {
  rm -f "$WORK/sound.wav"
  timeout 2 pw-record --target wprs-test-sink -P '{ stream.capture.sink=true }' --rate 48000 --channels 2 \
    --format s16 "$WORK/sound.wav" 2>/dev/null
  python3 -c "import array, math, sys, wave; w = wave.open(sys.argv[1]); a = array.array('h', w.readframes(w.getnframes()))[-48000:]
print(round(math.sqrt(sum(x * x for x in a) / len(a))) if len(a) == 48000 else -1)" "$WORK/sound.wav" 2>/dev/null || echo -1
}
between() { local l; l=$(level); [ "$l" -ge "$1" ] && [ "$l" -le "$2" ]; }
read -r _ _ _ s13 < <(ctl stream TEST-13 160 90)
ctl volume 100 && ctl set "$sound" -o TEST-13 && consume "$s13"
expect "s['outputs']['TEST-13']['playing']" True
plugin=$(gst-inspect-1.0 pipewiresink 2>/dev/null | sed -n 's/^ *Version *//p')
if printf '1.2\n%s\n' "$plugin" | sort -VC; then direct=true; else direct=false; fi
sounds() { ! $direct || wait_for 5 between 5000 20000; }
if $direct; then
  wait_for 10 grep -q "no audio output" "$WORK/wprsd.log" || fail "a video without an audio output was not reported"
  pw-cli create-node adapter '{ factory.name=support.null-audio-sink node.name=wprs-test-sink
    media.class=Audio/Sink object.linger=true audio.position=[FL FR] }' >/dev/null
fi
sounds || fail "no sound from the video in PipeWire: $(level)"
ctl pause && expect "s['outputs']['TEST-13']['playing']" False
sleep 6
ctl resume && expect "s['outputs']['TEST-13']['playing']" True
sounds || fail "the sound did not come back after a pause: $(level)"
"$BIN/probe" "$s13" 5 >/dev/null || fail "no frame after a pause of a video with sound"
pw-dump | python3 -c "import json, sys; nodes = [o['info']['props'] for o in json.load(sys.stdin) if o['type'].endswith('Node')]
piped = any(p.get('media.class') == 'Stream/Output/Audio' and p.get('node.name') == 'wallpaper-rs' for p in nodes)
sys.exit(piped != (sys.argv[1] == 'true'))" "$direct" || fail "PipeWire GStreamer plugin $plugin and direct sound disagree"
if $direct; then
  ctl volume 10
  wait_for 2 between 300 3000 || fail "the volume did not lower the sound: $(level)"
  ctl mute
  wait_for 2 between 0 20 || fail "muting left the sound on: $(level)"
fi
ctl volume 30 && ctl mute && leave
ctl release TEST-13 && expect "list(s['outputs'])" "[]"
echo "video sound checks passed"

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
