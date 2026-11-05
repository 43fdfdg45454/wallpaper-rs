#!/usr/bin/env bash
set -euo pipefail
tune=org.example.tune
media() { ctl media status --json | python3 -c "import json, sys; r = json.load(sys.stdin); print(eval(sys.argv[1]))" "$1"; }
says() { [ "$(media "$1")" = "$2" ]; }
mkdir -p "$WORK/tune" "$WORK/import" && cp "$ROOT/tests/fixtures/green-64x32.png" "$WORK/tune/preview.png"
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
  "layers": [{"id": "title", "type": "text", "text": {"media": "title"}}]}' 1.0.0)
read -r _ _ _ s21 < <(ctl stream TEST-21 160 90)
consume "$s21"
ctl lib apply "$uid" -o TEST-21
wait_for 10 reads "r['shown']['TEST-21']['media'], r['shown']['TEST-21']['granted']" "(True, [])" ||
  fail "the media scene was not declared: $(ctl input status --json)"
ctl media status | grep -q "^media player: not read" || fail "media status: $(ctl media status)"
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
leave && ctl release TEST-21 && bus Library1 Remove asb 1 "$uid" false >/dev/null
echo "media permission checks passed"
