#!/usr/bin/env bash
set -euo pipefail
tune=org.example.tune
media() { ctl media status --json | python3 -c "import json, sys; r = json.load(sys.stdin); print(eval(sys.argv[1]))" "$1"; }
says() { [ "$(media "$1")" = "$2" ]; }
mkdir -p "$WORK/tune" "$WORK/import" && cp "$ROOT/tests/fixtures/green-64x32.png" "$WORK/tune/preview.png"
png "$WORK/tune/red.png" ff0000 && png "$WORK/blue cover.png" 0000ff
art=$(python3 -c 'import pathlib, sys; print(pathlib.Path(sys.argv[1]).as_uri())' "$WORK/blue cover.png")
tuned() {
  printf '%s' "$1" >"$WORK/tune/scene.json"
  printf '{"schema": "wallpaper-rs.wpak/2", "id": "%s", "version": "%s", "title": "Tune", "license": "CC0-1.0",
    "kind": "scene", "entry": "scene.json", "previews": {"thumbnail": "preview.png"}, "origin": {"kind": "original"},
    "content": {"rating": "general"}, "photosensitive": false, "inputs": ["media"]}' "$tune" "$2" >"$WORK/tune/manifest.json"
  ctl pack build "$WORK/tune" -o "$WORK/import/tune-$2.wpak" >/dev/null
  local job
  job=$(bus Library1 Import asb 1 "$WORK/import/tune-$2.wpak" false | out "d[0]")
  wait_for 20 finished "$job" || fail "media scene import: $(bus Daemon1 GetJob t "$job")"
  reply "r['job']['output']['imported'][0]['uid']" Daemon1 GetJob t "$job"
}
uid=$(tuned '{"schema": "wallpaper-rs.scene/1", "canvas": {"size": [64, 32], "background": "#000000"},
  "assets": {"cover": {"path": "red.png", "media": "current"}},
  "layers": [{"id": "art", "type": "image", "image": "cover", "size": [64, 32], "transform": {"position": [32, 16]}},
    {"id": "title", "type": "text", "text": {"media": "title"}, "size": 4, "transform": {"position": [2, 2]}},
    {"id": "bar", "type": "solid", "color": "#00ff00", "size": [4, 2], "opacity": {"media": "progress"},
      "transform": {"position": [62, 31]}}]}' 1.0.0)
read -r _ _ _ s21 < <(ctl stream TEST-21 160 90)
consume "$s21"
ctl lib apply "$uid" -o TEST-21
wait_for 10 reads "r['shown']['TEST-21']['media'], r['shown']['TEST-21']['granted']" "(True, [])" ||
  fail "the media scene was not declared: $(ctl input status --json)"
ctl media status | grep -q "^media player: not read" || fail "media status: $(ctl media status)"
shown "$s21" 255 0 0
mkfifo "$WORK/player"
"$BIN/examples/player" example <"$WORK/player" >"$WORK/player.out" 2>&1 & pids+=($!)
exec 8>"$WORK/player"
wait_for 10 grep -q ready "$WORK/player.out" || fail "the example player did not start: $(cat "$WORK/player.out")"
play() { echo "$1" >&8; }
play '{"status": "Playing", "title": "Night Drive", "artist": ["Example Artist"], "album": "Example Album", "length": 200}'
sleep 1
says "r['listening']" False || fail "the player was read without permission: $(ctl media status --json)"
idle=$(ticks) && sleep 2 && idle=$(($(ticks) - idle))
[ "$idle" -lt 8 ] || fail "the daemon used $idle ticks in 2 s for a media scene without permission"
ctl input grant "$tune" media
wait_for 10 says "r['listening'], r['players'], r['playing'], r['track'], r['using']" "(True, 1, True, True, ['TEST-21'])" ||
  fail "the granted scene did not read the player: $(ctl media status --json)"
ctl media status | grep -qx "track: yes, playing: yes, cover: no" || fail "media status: $(ctl media status)"
shown "$s21" 255 0 0
play "{\"title\": \"Morning\", \"artist\": [\"Example Artist\"], \"length\": 100, \"art\": \"$art\"}"
wait_for 10 says "r['cover']" True || fail "the cover of the track was not read: $(ctl media status --json)"
shown "$s21" 0 0 255
play '{"title": "Evening", "artist": ["Example Artist"], "length": 100, "art": "https://example.org/cover.png"}'
wait_for 10 says "r['cover']" False || fail "a cover from the internet was read with the switch off"
shown "$s21" 255 0 0
play "{\"title\": \"Morning\", \"artist\": [\"Example Artist\"], \"length\": 100, \"art\": \"$art\"}"
shown "$s21" 0 0 255
uri() { python3 -c 'import pathlib, sys; print(pathlib.Path(sys.argv[1]).as_uri())' "$1"; }
song() { play "{\"title\": \"$1\", \"artist\": [\"Example Artist\"], \"length\": 100, \"art\": \"$2\"}"; }
png "$WORK/same.png" 00ff00 && song First "$(uri "$WORK/same.png")"
shown "$s21" 0 255 0
png "$WORK/same.png" ff00ff && song Second "$(uri "$WORK/same.png")"
shown "$s21" 255 0 255
mkdir -p "$WORK/site" && png "$WORK/site/cover.png" 00ffff
covers=$(python3 -c "import socket; s = socket.socket(); s.bind(('127.0.0.1', 0)); print(s.getsockname()[1])")
python3 -m http.server --bind 127.0.0.1 --directory "$WORK/site" "$covers" >"$WORK/covers.log" 2>&1 & pids+=($!)
wait_for 10 python3 -c "import urllib.request; urllib.request.urlopen('http://127.0.0.1:$covers/cover.png')" ||
  fail "the cover server did not start"
song Online "http://127.0.0.1:$covers/cover.png"
wait_for 10 says "r['cover']" False || fail "a cover from the internet was read with the switch off"
shown "$s21" 255 0 0
ctl config media.remote_covers true
wait_for 10 says "r['cover']" True || fail "turning media.remote_covers on did not fetch the cover of the playing track"
shown "$s21" 0 255 255
ctl config media.remote_covers false
wait_for 10 says "r['cover']" False || fail "turning media.remote_covers off kept the cover from the internet"
shown "$s21" 255 0 0
song Morning "$art"
shown "$s21" 0 0 255
more() { [ "$(grep -c '^gets' "$WORK/player.out")" -gt "$1" ]; }
asked() {
  local n
  n=$(grep -c '^gets' "$WORK/player.out" || true)
  play '{"gets": true}' && wait_for 5 more "$n" && sed -n 's/^gets //p' "$WORK/player.out" | tail -1
}
before=$(asked) && sleep 3 && after=$(asked)
[ "$before" = "$after" ] || fail "the position was asked $((after - before)) times in 3 s while the track played"
play '{"seek": 50}'
play '{"status": "Paused"}'
wait_for 10 says "r['playing']" False || fail "a pause was not followed: $(ctl media status --json)"
idle=$(ticks) && sleep 2 && idle=$(($(ticks) - idle))
[ "$idle" -lt 8 ] || fail "the daemon used $idle ticks in 2 s for a paused track with a progress bar"
play '{"quit": true}'
exec 8>&-
wait_for 10 says "r['players'], r['track']" "(0, False)" || fail "a player that left was kept: $(ctl media status --json)"
idle=$(ticks) && sleep 2 && idle=$(($(ticks) - idle))
[ "$idle" -lt 8 ] || fail "the daemon used $idle ticks in 2 s while no media player was open"
ctl config media.remote_covers true
wait_for 10 says "r['remote_covers']" True || fail "the switch for covers from the internet: $(ctl media status --json)"
ctl config media.remote_covers false
ctl input revoke "$tune" media
wait_for 10 says "r['listening']" False || fail "the player was read after the permission was taken back"
shown "$s21" 255 0 0
leave && ctl release TEST-21 && bus Library1 Remove asb 1 "$uid" false >/dev/null
tuning=$WORK/scene/tuning
ctl scene new "$tuning" >/dev/null
ctl scene set "$tuning" /manifest/inputs '["media"]'
ctl scene set "$tuning" /scene/canvas/background '#000000'
ctl scene import "$tuning" "$WORK/tune/red.png" --id red >/dev/null
ctl scene asset "$tuning" cover --cover current --placeholder red || fail "no cover asset was made"
if ctl scene asset "$tuning" other --cover current 2>/dev/null; then fail "a cover without a placeholder was made"; fi
ctl scene add "$tuning" /scene/layers/- '{"id": "art", "type": "image", "image": "cover", "size": [1920, 1080], "transform": {"position": [960, 540]}}'
ctl scene add "$tuning" /scene/layers/- '{"id": "title", "type": "text", "text": {"media": "title"}, "size": 40, "transform": {"position": [40, 40]}}'
ctl scene add "$tuning" /scene/layers/- '{"id": "before", "type": "image", "image": "red", "size": [8, 8], "visible": false}'
ctl scene set "$tuning" /scene/layers/1/text/placeholder '"Nothing plays"'
ctl scene translate "$tuning" /scene/layers/1/text/placeholder es "No suena nada"
ctl scene set "$tuning" /scene/layers/1/text/placeholder "\"$(printf 'x%.0s' $(seq 600))\""
if ctl pack check "$tuning" >/dev/null 2>&1; then fail "a placeholder of 600 characters passed the check"; fi
ctl scene undo "$tuning"
ctl scene import "$tuning" "$WORK/tune/red.png" --id previous_cover >/dev/null
ctl scene asset "$tuning" previous_cover --cover previous --placeholder red --field /scene/layers/2/image
doc() { python3 -c "import json, sys; d = json.load(open(sys.argv[2])); print(eval(sys.argv[1]))" "$1" "$tuning/scene.json"; }
[ "$(doc "d['layers'][1]['text']['placeholder']")" = "{'en': 'Nothing plays', 'es': 'No suena nada'}" ] ||
  fail "the text while nothing plays: $(doc "d['layers'][1]['text']")"
[ "$(doc "'media' in d['assets']['previous_cover'], d['assets'][d['layers'][2]['image']]['media']")" = "(False, 'previous')" ] ||
  fail "choosing the previous cover for a field: $(doc "d['assets'], d['layers'][2]")"
if ctl scene asset "$tuning" cover --cover none --field /scene/layers/2/image 2>/dev/null; then fail "a field took no cover"; fi
cp "$WORK/blue cover.png" "$tuning/blue.png"
printf '{"title": "Night Drive", "artist": ["Example Artist"], "playing": true, "position": 5, "duration": 100, "cover": "blue.png"}' >"$WORK/track.json"
read -r _ _ _ s22 < <(ctl stream preview:2 160 90)
consume "$s22"
ctl scene preview "$tuning" -o preview:2 --time 0 --media "$WORK/track.json" || fail "a made-up track was not previewed"
ctl media status | grep -q "^media player: not read" || fail "a made-up track read the media player: $(ctl media status)"
shown "$s22" 0 0 255
ctl scene preview "$tuning" -o preview:2 --time 0 || fail "the scene was not previewed without a made-up track"
shown "$s22" 255 0 0
printf '{"cover": "../blue cover.png"}' >"$WORK/outside.json"
if ctl scene preview "$tuning" -o preview:2 --media "$WORK/outside.json" 2>/dev/null; then fail "a cover outside the folder was read"; fi
ctl scene set "$tuning" /scene/layers/0 '{"id": "art", "type": "solid", "color": {"media": "primary"}, "size": [1920, 1080], "transform": {"position": [960, 540]}}'
ctl scene preview "$tuning" -o preview:2 --time 0 --media "$WORK/track.json"
shown "$s22" 0 0 255
ctl scene preview "$tuning" -o preview:2 --time 0
shown "$s22" 255 0 0
ctl scene set "$tuning" /scene/layers/0/visible '{"media": "playing"}'
ctl scene preview "$tuning" -o preview:2 --time 0 --media "$WORK/track.json"
shown "$s22" 0 0 255
printf '{"title": "Night Drive", "playing": false, "cover": "blue.png"}' >"$WORK/paused.json"
ctl scene preview "$tuning" -o preview:2 --time 0 --media "$WORK/paused.json"
shown "$s22" 0 0 0
printf '{"title": "%s"}' "$(printf 'x%.0s' $(seq 600))" >"$WORK/long.json"
if ctl scene preview "$tuning" -o preview:2 --media "$WORK/long.json" 2>"$WORK/long.err"; then fail "a title of 600 characters was previewed"; fi
grep -q "512" "$WORK/long.err" || fail "the long title was refused without saying why: $(cat "$WORK/long.err")"
printf '[1, 2]' >"$WORK/list.json"
if ctl scene preview "$tuning" -o preview:2 --media "$WORK/list.json" 2>/dev/null; then fail "a list was taken for a made-up track"; fi
ctl scene preview "$tuning" -o preview:2 --stop
leave && ctl release preview:2
echo "media permission checks passed"
