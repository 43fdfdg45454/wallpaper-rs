#!/usr/bin/env bash
set -euo pipefail

BIN=$(realpath "${1:?usage: $0 <bin-dir>}")
ROOT=$(cd "$(dirname "$0")/.." && pwd)
WORK=$(mktemp -d /tmp/wprs-sync.XXXXXX)
export PIPEWIRE_RUNTIME_DIR=$WORK/run XDG_RUNTIME_DIR=$WORK/run
mkdir -p "$XDG_RUNTIME_DIR" && chmod 700 "$XDG_RUNTIME_DIR"

pids=()
cleanup() {
  for p in "${pids[@]}"; do kill "$p" 2>/dev/null || true; done
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
fail() { echo "FAIL: $*"; exit 1; }
jq_() { python3 -c "import json, sys; r = json.load(sys.stdin); print(eval(sys.argv[1]))" "$1"; }

port=$(python3 -c "import socket; s = socket.socket(); s.bind(('127.0.0.1', 0)); print(s.getsockname()[1])")
URL=http://127.0.0.1:$port
export WPRS_DATA=$WORK/server WPRS_LISTEN=127.0.0.1:$port WPRS_PUBLIC_URL=$URL WPRS_RATE_LIMITS=false
export WPRS_AUTO_PUBLISH=never WPRS_PUBLISH_DELAY_SECS=1 WPRS_LOG=warn
"$BIN/wprs-server" repo init --name Example >/dev/null
"$BIN/wprs-server" serve >"$WORK/server.log" 2>&1 & pids+=($!)
wait_for 20 curl -fsS "$URL/healthz" || fail "the server did not start"
jar=$WORK/cookies
curl -fsS -o /dev/null -c "$jar" --data-urlencode "token=$("$BIN/wprs-server" setup-token)" \
  -d handle=admin -d password=correct-horse-battery "$URL/setup"
api() { curl -fsS -b "$jar" -H 'content-type: application/json' "$@"; }
unset WPRS_DATA WPRS_LISTEN WPRS_PUBLIC_URL WPRS_RATE_LIMITS WPRS_AUTO_PUBLISH WPRS_PUBLISH_DELAY_SECS WPRS_LOG

keyring() {
  env "$1" dbus-send --session --print-reply --dest=org.freedesktop.DBus /org/freedesktop/DBus \
    org.freedesktop.DBus.NameHasOwner string:org.freedesktop.secrets | grep -q 'boolean true'
}

device() {
  local dir=$WORK/$1
  mkdir -p "$dir/run" && chmod 700 "$dir/run"
  pids+=("$(dbus-daemon --session --fork --address="unix:path=$dir/bus" --print-pid)")
  local env=(DBUS_SESSION_BUS_ADDRESS="unix:path=$dir/bus" XDG_RUNTIME_DIR="$dir/run" XDG_CONFIG_HOME="$dir/config"
    XDG_DATA_HOME="$dir/data" XDG_CACHE_HOME="$dir/cache")
  printf 'keyring password' | env "${env[@]}" gnome-keyring-daemon --unlock --components=secrets --foreground \
    >"$WORK/keyring-$1.log" 2>&1 & pids+=($!)
  wait_for 10 keyring "${env[0]}" || fail "the keyring of $1 did not start"
  env "${env[@]}" RUST_LOG=info,naga=warn,wgpu_core=warn,wgpu_hal=warn "$BIN/wprsd" >"$WORK/wprsd-$1.log" 2>&1 &
  pids+=($!)
  wait_for 30 env "${env[@]}" "$BIN/wprs-ctl" state || fail "daemon $1 did not start"
}
a() { DBUS_SESSION_BUS_ADDRESS=unix:path=$WORK/a/bus "$BIN/wprs-ctl" "$@"; }
b() { DBUS_SESSION_BUS_ADDRESS=unix:path=$WORK/b/bus "$BIN/wprs-ctl" "$@"; }
finished() { [ "$("$1" job "$2" --json | jq_ "r['job']['state']")" = "done" ]; }
await() { wait_for "${3:-60}" finished "$1" "$2" || fail "job $2 of $1: $("$1" job "$2")"; }
repos() { [ "$(a remote ls --json | jq_ "len(r['repos'])")" = 1 ]; }
value() { "$1" server status --json | jq_ "r['account']$2"; }

pids+=("$(dbus-daemon --session --fork --address="unix:path=$WORK/bus" --print-pid)")
export DBUS_SESSION_BUS_ADDRESS=unix:path=$WORK/bus
pipewire >"$WORK/pipewire.log" 2>&1 & pids+=($!)
wait_for 10 test -S "$PIPEWIRE_RUNTIME_DIR/pipewire-0" || fail "PipeWire did not start"
wireplumber >"$WORK/wireplumber.log" 2>&1 & pids+=($!)
device a
device b
a server add "$URL" --wait >/dev/null
b server add "$URL" --wait >/dev/null

link=$(a server link --name Desk)
code=$(sed -n 's/^code = //p' <<<"$link")
api -d "{\"user_code\": \"$code\", \"approve\": true}" "$URL/api/v1/auth/device/approve" >/dev/null
await a "$(tail -n 1 <<<"$link")"
[ "$(value a "['state'], r['account']['encrypted'], r['account']['device']")" = "('ready', True, 'Device 1')" ] ||
  fail "first device: $(a server status --json)"
wait_for 30 repos || fail "the repository of the server was not added: $(a remote ls)"

link=$(b server link --name Laptop)
a server approve "$(sed -n 's/^code = //p' <<<"$link")" --wait >/dev/null
await b "$(tail -n 1 <<<"$link")"
[ "$(value b "['state']")" = needs_key ] || fail "second device: $(b server status --json)"
join=$(b server join "Device 1")
a server share "Device 2" "$(sed -n 's/^code = //p' <<<"$join")" --wait >/dev/null
await b "$(tail -n 1 <<<"$join")"
[ "$(value b "['state']")" = ready ] || fail "the key share: $(b server status --json)"
if a server share "Device 2" "0000 0000 0000" --wait 2>"$WORK/share.out"; then fail "a wrong code shared the key"; fi
grep -q "codes differ" "$WORK/share.out" || fail "wrong code: $(cat "$WORK/share.out")"
echo "link and key share checks passed"

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
case_id=$(api "$URL/api/v1/mod/queue" | jq_ "r['items'][0]['id']")
api -d '{"decision": "approve", "statement": "Looks fine to publish."}' "$URL/api/v1/mod/cases/$case_id/decision" >/dev/null
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
[ "$(b playlist show "Quiet evenings" --json | jq_ "r['total']")" = 1 ] || fail "the playlist entry did not arrive"
[ "$(value b "['devices'][0]['label']")" = Desk ] || fail "device names: $(b server status --json)"
b config models.allow_download false
a config pause.on_battery false
for d in a b a; do "$d" server sync --wait >/dev/null; done
[ "$(b config pause.on_battery)" = "pause.on_battery = false" ] || fail "a setting did not arrive"
[ "$(a config models.allow_download)/$(b config models.allow_download)" = \
  "models.allow_download = true/models.allow_download = false" ] || fail "the download switch left its device"
echo "sync checks passed"

sql() { python3 -c "import sqlite3, sys; print(*sqlite3.connect(sys.argv[1]).execute(sys.argv[2]).fetchone())" \
  "$WORK/server/db/wprs.sqlite" "$1"; }
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
  "{\"epoch\": $epoch, \"changes\": [{\"collection\": \"playlists\", \"item\": \"$list\", \"base\": $seq, \"deleted\": true}]}" \
  >/dev/null
b server sync --wait >/dev/null
[ "$(b playlist ls --json | jq_ "len(r['playlists'])")" = 1 ] || fail "a tombstone without payload deleted a playlist"
echo "idle and tombstone checks passed"

python3 - "$WORK/server/db/wprs.sqlite" <<'CHECK' || fail "the server holds readable sync data"
import sqlite3, sys
db = sqlite3.connect(sys.argv[1])
rows = db.execute("SELECT collection, item, payload FROM sync_records").fetchall()
assert {"library", "playlists", "settings", "device"} <= {r[0] for r in rows}, rows
for collection, item, payload in rows:
    assert payload[0] == 1 and (len(payload) - 41) % 256 == 0, (collection, item)
    for word in (b"evening", b"Quiet", b"Desk", b"Laptop", b"dusk"):
        assert word not in payload and word.decode() not in item, (collection, item, word)
CHECK
for f in "$WORK"/server/db/* ; do
  if grep -q -a -e evening-glow -e 'Quiet evenings' -e Laptop "$f"; then fail "$f holds plain text"; fi
done
if grep -r -q -s -a -e 'wprs_d_' "$WORK/a/data" "$WORK/a/config" "$WORK/b/data" "$WORK"/wprsd-*.log; then
  fail "a token reached a file or a log"
fi
a server remove --wipe
[ "$(value a "")" = None ] || fail "the server was kept"
status=0
echo "server sync test passed"
