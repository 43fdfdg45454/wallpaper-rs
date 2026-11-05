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
bus Library1 Remove asb 1 "$cue_uid" false >/dev/null
heard=org.example.heard
mkdir -p "$WORK/heard/scripts" && cp "$WORK/cue/preview.png" "$WORK/heard/preview.png"
printf '%s' 'import { scene, media } from "wprs";
export function mediaPlayback({ state }) { scene.layer("box").color = state === "playing" && media.title ? "#0000ff" : "#00ff00"; }' >"$WORK/heard/scripts/heard.js"
printf '%s' '{"schema": "wallpaper-rs.scene/1", "canvas": {"size": [64, 32], "background": "#000000"},
  "scripts": [{"path": "scripts/heard.js", "events": ["mediaPlayback"], "uses": ["media"]}],
  "layers": [{"id": "box", "type": "solid", "size": [64, 32], "color": "#ff0000", "transform": {"position": [32, 16]}}]}' >"$WORK/heard/scene.json"
sed "s/$cue/$heard/; s/\"Cue\"/\"Heard\"/" "$WORK/cue/manifest.json" >"$WORK/heard/manifest.json"
ctl pack build "$WORK/heard" -o "$WORK/import/heard.wpak" >/dev/null
job=$(bus Library1 Import asb 1 "$WORK/import/heard.wpak" false | out "d[0]")
wait_for 20 finished "$job" || fail "media script scene import: $(bus Daemon1 GetJob t "$job")"
heard_uid=$(reply "r['job']['output']['imported'][0]['uid']" Daemon1 GetJob t "$job")
ctl lib apply "$heard_uid" -o TEST-23
shown "$s23" 255 0 0
cp "$WORK/blue cover.png" "$WORK/heard/blue.png"
printf '{"title": "Night Drive", "playing": true, "duration": 100, "cover": "blue.png"}' >"$WORK/heard/playing.json"
printf '{"title": "Night Drive", "playing": false, "duration": 100, "cover": "blue.png"}' >"$WORK/heard/paused.json"
ctl media simulate TEST-23 "$WORK/heard/playing.json" || fail "the made-up track was refused"
shown "$s23" 0 0 255
ctl media status | grep -q "^media player: not read" || fail "a made-up track read the media player: $(ctl media status)"
ctl media simulate TEST-23 "$WORK/heard/paused.json"
shown "$s23" 0 255 0
printf '{"title": "%s"}' "$(printf 'x%.0s' $(seq 600))" >"$WORK/heard/long.json"
if ctl media simulate TEST-23 "$WORK/heard/long.json" 2>/dev/null; then fail "a made-up title of 600 characters was taken"; fi
ctl media simulate TEST-23 --stop
idle=$(ticks) && sleep 2 && idle=$(($(ticks) - idle))
[ "$idle" -lt 8 ] || fail "the daemon used $idle ticks in 2 s for a media script without a player"
leave && ctl release TEST-23 && bus Library1 Remove asb 1 "$heard_uid" false >/dev/null
echo "media trigger and script checks passed"
