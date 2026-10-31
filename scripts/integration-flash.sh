#!/usr/bin/env bash
set -euo pipefail
declare storm serial7
mkdir -p "$WORK/flash" && cp "$ROOT/tests/fixtures/green-64x32.png" "$WORK/flash/preview.png"
echo '{"schema": "wallpaper-rs.scene/1", "canvas": {"size": [64, 32]}, "layers": [{"id": "fill", "type": "solid", "color": "#ffffff"}]}' >"$WORK/flash/scene.json"
flash() {
  printf '{"schema": "wallpaper-rs.wpak/2", "id": "%s", "version": "1.0.0", "title": "Flash", "license": "CC0-1.0",
    "kind": "scene", "entry": "scene.json", "previews": {"thumbnail": "preview.png"}, "origin": {"kind": "original"},
    "content": {"rating": "general"}, "photosensitive": true}' "$1" >"$WORK/flash/manifest.json"
  ctl pack build "$WORK/flash" -o "$WORK/import/flash.wpak" >/dev/null
  job=$(bus Library1 Import asb 1 "$WORK/import/flash.wpak" false | out "d[0]")
  wait_for 20 finished "$job" || fail "flashing scene import: $(bus Daemon1 GetJob t "$job")"
}
flash org.example.flash
flashing=$(reply "r['items'][0]['uid']" Library1 Search suu "flash" 0 1)
ctl lib confirm photosensitive "$flashing" && ctl lib apply "$flashing" -o TEST-7
stormy=$(ctl profile save Stormy --no-options)
flash org.example.flash2
ctl profile use "$stormy"
expect "s['outputs']['TEST-7']['item'], 'once you allow it' in (s['outputs']['TEST-7']['error'] or '')" "(None, True)"
ctl stream TEST-20 160 90 >/dev/null && ctl background '#203040' -o TEST-20 && ctl lib confirm photosensitive "$storm"
expect "s['outputs']['TEST-7']['item'], s['outputs']['TEST-20']['background']" "(None, '#203040')"
ctl lib confirm photosensitive "$flashing"
expect "s['outputs']['TEST-7']['item'], s['outputs']['TEST-7']['error'], s['outputs']['TEST-20']['background']" "('$flashing', None, '#203040')"
ctl release TEST-20 && ctl profile off && ctl profile rm "$stormy"
ctl lib apply "$storm" -o TEST-7
expect "s['outputs']['TEST-7']['item'], s['outputs']['TEST-7']['error']" "('$storm', None)"
mkdir -p "$WORK/blink" && cp "$WORK/flash/preview.png" "$WORK/blink/preview.png"
cat >"$WORK/blink/scene.json" <<'BLINK'
{"schema": "wallpaper-rs.scene/1", "canvas": {"size": [64, 32], "background": "#000000"},
 "timelines": [{"id": "blink", "duration": 0.1, "loop": "repeat"}],
 "layers": [{"id": "fill", "type": "solid", "color": "#ffffff",
   "opacity": {"timeline": "blink", "keys": [[0, 0], [0.05, 1], [0.1, 0]]}}]}
BLINK
sed 's/"org.example.flash2"/"org.example.blink"/' "$WORK/flash/manifest.json" >"$WORK/blink/manifest.json"
ctl pack build "$WORK/blink" -o "$WORK/import/blink.wpak" >/dev/null
job=$(bus Library1 Import asb 1 "$WORK/import/blink.wpak" false | out "d[0]")
wait_for 20 finished "$job" || fail "blinking scene import: $(bus Daemon1 GetJob t "$job")"
blink=$(reply "r['job']['output']['imported'][0]['uid']" Daemon1 GetJob t "$job")
ctl lib confirm photosensitive "$blink" && ctl lib apply "$blink" -o TEST-7 && consume "$serial7"
calmed() { [ -n "$(field "s['outputs']['TEST-7']['flashes']")" ]; }
sleep 2
if calmed; then fail "the flash guard acted while it was off: $(ctl state)"; fi
ctl config playback.flash_guard true
wait_for 20 calmed || fail "the flash guard let a scene flash ten times a second: $(ctl state)"
ctl config playback.flash_guard false
expect "s['outputs']['TEST-7']['flashes']" ""
expect "s['outputs']['TEST-7']['left_out']" "[]"
ctl retry-effects || fail "retry-effects failed with a scene shown"
expect "s['outputs']['TEST-7']['left_out']" "[]"
leave && bus Library1 Remove asb 1 "$blink" false >/dev/null
ctl lib apply "$storm" -o TEST-7
echo "flash guard checks passed"
bus Library1 Remove asb 1 "$flashing" false >/dev/null
