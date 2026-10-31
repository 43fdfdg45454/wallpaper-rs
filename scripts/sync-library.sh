#!/usr/bin/env bash
set -euo pipefail
pkg=$WORK/pkg
mkdir -p "$pkg/src"
cp "$ROOT/tests/fixtures/green-64x32.png" "$pkg/src/image.png"
cp "$ROOT/tests/fixtures/green-64x32.png" "$pkg/src/preview.png"
cat >"$pkg/src/manifest.json" <<'MANIFEST'
{"schema": "wallpaper-rs.wpak/1", "id": "org.example.dusk", "version": "1.0.0", "title": "Dusk",
 "license": "CC0-1.0", "kind": "image", "entry": "image.png", "preview": "preview.png", "tags": ["sky"]}
MANIFEST
a pack build "$pkg/src" -o "$pkg/dusk.wpak" >/dev/null
a server upload "$pkg/dusk.wpak" --wait --json | jq_ "r['output']['state']" | grep -qx pending_review ||
  fail "the upload did not wait for review"
case_id=$(review "$URL/api/v1/mod/queue" | jq_ "r['items'][0]['id']")
if api -d '{"decision": "approve"}' "$URL/api/v1/mod/cases/$case_id/decision" 2>/dev/null; then
  fail "a moderator approved their own upload"
fi
review -d '{"decision": "approve", "statement": "Looks fine to publish."}' "$URL/api/v1/mod/cases/$case_id/decision" >/dev/null
api -X POST "$URL/api/v1/admin/repo/publish" >/dev/null
a remote refresh --wait >/dev/null
a pkg install org.example.dusk --wait >/dev/null
item=$(a pkg ls --json | jq_ "r['packages'][0]['item']")
a lib tag "$item" --add evening-glow >/dev/null
a playlist new "Quiet evenings" >/dev/null
a playlist add "Quiet evenings" "$item" >/dev/null
a server sync --wait >/dev/null
b server sync --wait >/dev/null
[ "$(b playlist ls --json | jq_ "[p['name'] for p in r['playlists']]")" = "['Quiet evenings']" ] ||
  fail "the playlist did not arrive: $(b playlist ls)"
b remote refresh --wait >/dev/null
b pkg install org.example.dusk --wait >/dev/null
b server sync --wait >/dev/null
[ "$(b lib search tag:evening-glow --json | jq_ "r['total']")" = 1 ] || fail "the tag did not arrive: $(b lib search '')"
cp "$ROOT/tests/fixtures/bands-640x180.png" "$WORK/gone.png"
for d in a b; do "$d" lib import "$WORK/gone.png" --wait >/dev/null; done
a lib tag "$(a lib search gone --json | jq_ "r['items'][0]['uid']")" --add to-delete >/dev/null
for d in a b; do "$d" server sync --wait >/dev/null; done
gone() { "$1" lib search gone --json | jq_ "r['total']"; }
[ "$(gone a)/$(gone b)" = 1/1 ] || fail "the item to delete: $(b lib search gone)"
[ "$(b lib search tag:to-delete --json | jq_ "r['total']")" = 1 ] || fail "the tag of the item to delete did not arrive"
a lib rm "$(a lib search gone --json | jq_ "r['items'][0]['uid']")" >/dev/null
a server sync --wait >/dev/null
b server sync --wait >/dev/null
[ "$(gone b)" = 0 ] || fail "a deletion did not reach the other device: $(b lib search gone)"
for d in a b a b; do "$d" server sync --wait >/dev/null; done
[ "$(gone a)/$(gone b)" = 0/0 ] || fail "a deleted item came back: $(a lib search gone) $(b lib search gone)"
echo "deletion checks passed"
write "UPDATE sync_heads SET format = format + 1"
if a server sync --wait 2>"$WORK/old-a.out"; then fail "an older client synced"; fi
if b server sync --wait 2>"$WORK/old-b.out"; then fail "an older client synced"; fi
grep -q "synced by a newer version of wallpaper-rs" "$WORK/old-a.out" || fail "the outdated client: $(cat "$WORK/old-a.out")"
grep -q "una versión más nueva de wallpaper-rs" "$WORK/old-b.out" || fail "the outdated client in Spanish: $(cat "$WORK/old-b.out")"
write "UPDATE sync_heads SET format = format - 1"
echo "sync format checks passed"
[ "$(b playlist show "Quiet evenings" --json | jq_ "r['total']")" = 1 ] || fail "the playlist entry did not arrive"
[ "$(value b "['devices'][0]['label']")" = Desk ] || fail "device names: $(b server status --json)"
b config models.allow_download false
b config playback.software_parallax true
a config pause.on_battery false
a profile save Calm >/dev/null
for d in a b a; do "$d" server sync --wait >/dev/null; done
[ "$(b config pause.on_battery)" = "pause.on_battery = false" ] || fail "a setting did not arrive"
[ "$(a config models.allow_download)/$(b config models.allow_download)" = \
  "models.allow_download = true/models.allow_download = false" ] || fail "the download switch left its device"
[ "$(a config playback.software_parallax)" = "playback.software_parallax = false" ] ||
  fail "a device setting left its device"
options="sorted(set(r['profile']['options']) & {'playback.volume', 'playback.max_fps', 'library.thumb_cache_mb'})"
[ "$(b profile show Calm --json | jq_ "$options")" = "['playback.max_fps']" ] ||
  fail "the profile did not arrive without device settings: $(b profile show Calm --json)"
tint=$WORK/tint
mkdir -p "$tint/src"
cp "$ROOT/tests/fixtures/green-64x32.png" "$tint/src/preview.png"
cat >"$tint/src/manifest.json" <<'MANIFEST'
{"schema": "wallpaper-rs.wpak/1", "id": "org.example.tint", "version": "1.0.0", "title": "Tint",
 "license": "CC0-1.0", "kind": "scene", "entry": "scene.json", "preview": "preview.png",
 "properties": {"tint": {"type": "color", "label": "Tint", "default": "#0000ff"}}}
MANIFEST
cat >"$tint/src/scene.json" <<'SCENE'
{"schema": "wallpaper-rs.scene/1", "canvas": {"size": [64, 32]},
 "layers": [{"id": "tint", "type": "solid", "color": {"prop": "tint"}, "size": [64, 32]}]}
SCENE
a pack build "$tint/src" -o "$tint/tint.wpak" >/dev/null
for d in a b; do
  "$d" lib import "$tint/tint.wpak" --wait >/dev/null
  "$d" lib apply "$("$d" lib search kind:scene --json | jq_ "r['items'][0]['uid']")"
done
a prop preset save Mine >/dev/null
for d in a b; do "$d" server sync --wait >/dev/null; done
settings() { b prop preset ls --json | jq_ "[p['label'] for p in r['user_presets']]"; }
[ "$(settings)" = "['Mine']" ] || fail "the preset of settings did not arrive: $(b prop preset ls)"
a prop preset rename Mine Leaf
for d in a b; do "$d" server sync --wait >/dev/null; done
[ "$(settings)" = "['Leaf']" ] || fail "the renamed preset kept its old name: $(b prop preset ls)"
echo "sync checks passed"

sleep 8
before=$(sql "SELECT max(seq) FROM sync_records")
sleep 12
[ "$(sql "SELECT max(seq) FROM sync_records")" = "$before" ] || fail "the devices keep syncing while idle"
start=$(curl -fsS -H 'content-type: application/json' -d '{"platform": "other"}' "$URL/api/v1/auth/device/start")
api -d "{\"user_code\": \"$(jq_ "r['user_code']" <<<"$start")\", \"approve\": true}" \
  "$URL/api/v1/auth/device/approve" >/dev/null
rogue=$(curl -fsS -H 'content-type: application/json' -d "{\"device_code\": \"$(jq_ "r['device_code']" <<<"$start")\"}" \
  "$URL/api/v1/auth/device/token" | jq_ "r['token']")
read -r list seq < <(sql "SELECT item, seq FROM sync_records WHERE collection = 'playlists'")
epoch=$(curl -fsS -H "authorization: Bearer $rogue" "$URL/api/v1/sync/state" | jq_ "r['epoch']")
curl -fsS -H "authorization: Bearer $rogue" -H 'content-type: application/json' "$URL/api/v1/sync/changes" -d \
  "{\"epoch\": $epoch, \"format\": 1, \"changes\": [{\"collection\": \"playlists\", \"item\": \"$list\", \"base\": $seq, \"deleted\": true},
  {\"collection\": \"settings\", \"item\": \"transition\", \"base\": 0, \"deleted\": true}]}" >/dev/null
b server sync --wait >/dev/null
[ "$(b playlist ls --json | jq_ "len(r['playlists'])")" = 1 ] || fail "a tombstone without payload deleted a playlist"
a server sync --wait >/dev/null
sleep 8
before=$(sql "SELECT max(seq) FROM sync_records")
sleep 12
[ "$(sql "SELECT max(seq) FROM sync_records")" = "$before" ] || fail "the devices keep deleting a record of an old name"
echo "idle and tombstone checks passed"
