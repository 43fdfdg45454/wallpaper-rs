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
    {"id": "title", "type": "text", "text": {"media": "title"}, "size": 4, "transform": {"position": [2, 2]}}]}' 1.0.0)
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
play '{"status": "Paused"}'
wait_for 10 says "r['playing']" False || fail "a pause was not followed: $(ctl media status --json)"
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
printf '{"title": "%s"}' "$(printf 'x%.0s' $(seq 600))" >"$WORK/long.json"
if ctl scene preview "$tuning" -o preview:2 --media "$WORK/long.json" 2>"$WORK/long.err"; then fail "a title of 600 characters was previewed"; fi
grep -q "512" "$WORK/long.err" || fail "the long title was refused without saying why: $(cat "$WORK/long.err")"
printf '[1, 2]' >"$WORK/list.json"
if ctl scene preview "$tuning" -o preview:2 --media "$WORK/list.json" 2>/dev/null; then fail "a list was taken for a made-up track"; fi
ctl scene preview "$tuning" -o preview:2 --stop
leave && ctl release preview:2
echo "media permission checks passed"
