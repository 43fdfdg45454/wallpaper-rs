#!/usr/bin/env bash
set -euo pipefail
cue=org.example.cue
mkdir -p "$WORK/cue" && cp "$ROOT/tests/fixtures/green-64x32.png" "$WORK/cue/preview.png"
printf '%s' '{"schema": "wallpaper-rs.scene/1", "canvas": {"size": [64, 32], "background": "#000000"},
  "timelines": [{"id": "song", "duration": 0.2, "autoplay": false, "trigger": "media:track"},
    {"id": "rest", "duration": 0.2, "autoplay": false, "trigger": "media:pause"}],
  "layers": [{"id": "glow", "type": "solid", "size": [64, 32], "transform": {"position": [32, 16]},
      "color": {"timeline": "song", "keys": [[0, "#ff0000"], [0.1, "#0000ff"]]}},
    {"id": "dim", "type": "solid", "size": [64, 32], "transform": {"position": [32, 16]}, "color": "#00ff00",
      "opacity": {"timeline": "rest", "keys": [[0, 0], [0.1, 1]]}}]}' >"$WORK/cue/scene.json"
printf '{"schema": "wallpaper-rs.wpak/2", "id": "%s", "version": "1.0.0", "title": "Cue", "license": "CC0-1.0",
  "kind": "scene", "entry": "scene.json", "previews": {"thumbnail": "preview.png"}, "origin": {"kind": "original"},
  "content": {"rating": "general"}, "photosensitive": false, "inputs": ["media"]}' "$cue" >"$WORK/cue/manifest.json"
ctl pack build "$WORK/cue" -o "$WORK/import/cue.wpak" >/dev/null
job=$(bus Library1 Import asb 1 "$WORK/import/cue.wpak" false | out "d[0]")
wait_for 20 finished "$job" || fail "media cue scene import: $(bus Daemon1 GetJob t "$job")"
cue_uid=$(reply "r['job']['output']['imported'][0]['uid']" Daemon1 GetJob t "$job")
read -r _ _ _ s23 < <(ctl stream TEST-23 160 90)
consume "$s23"
ctl lib apply "$cue_uid" -o TEST-23
shown "$s23" 255 0 0
mkfifo "$WORK/cue-player"
"$BIN/examples/player" cues <"$WORK/cue-player" >"$WORK/cue-player.out" 2>&1 & pids+=($!)
exec 9>"$WORK/cue-player"
wait_for 10 grep -q ready "$WORK/cue-player.out" || fail "the example player did not start: $(cat "$WORK/cue-player.out")"
echo '{"status": "Playing", "title": "Night Drive", "artist": ["Example Artist"], "length": 200}' >&9
sleep 1
shown "$s23" 255 0 0
idle=$(ticks) && sleep 2 && idle=$(($(ticks) - idle))
[ "$idle" -lt 8 ] || fail "the daemon used $idle ticks in 2 s for media triggers without permission"
ctl input grant "$cue" media
shown "$s23" 0 0 255
echo '{"status": "Paused"}' >&9
shown "$s23" 0 255 0
idle=$(ticks) && sleep 2 && idle=$(($(ticks) - idle))
[ "$idle" -lt 8 ] || fail "the daemon used $idle ticks in 2 s after the media triggers ran"
echo '{"quit": true}' >&9
exec 9>&-
ctl input revoke "$cue" media
leave && ctl release TEST-23 && bus Library1 Remove asb 1 "$cue_uid" false >/dev/null
echo "media trigger checks passed"
