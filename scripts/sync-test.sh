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
invite=$(api -d '{"role": "moderator"}' "$URL/api/v1/admin/invites" | python3 -c "import json, sys; print(json.load(sys.stdin)['code'])")
curl -fsS -o /dev/null -H 'content-type: application/json' "$URL/api/v1/auth/register" \
  -d "{\"handle\": \"reviewer\", \"password\": \"another-horse-battery\", \"invite\": \"$invite\"}"
curl -fsS -o /dev/null -c "$WORK/reviewer" -d handle=reviewer -d password=another-horse-battery "$URL/login"
review() { curl -fsS -b "$WORK/reviewer" -H 'content-type: application/json' "$@"; }
unset WPRS_DATA WPRS_LISTEN WPRS_PUBLIC_URL WPRS_RATE_LIMITS WPRS_AUTO_PUBLISH WPRS_PUBLISH_DELAY_SECS WPRS_LOG

keyring() {
  env "$1" dbus-send --session --print-reply --dest=org.freedesktop.DBus /org/freedesktop/DBus \
    org.freedesktop.DBus.NameHasOwner string:org.freedesktop.secrets | grep -q 'boolean true'
}

declare -A daemons
daemon() {
  local dir=$WORK/$1 name=$1 more
  mapfile -t more <"$dir/env"
  local env=(DBUS_SESSION_BUS_ADDRESS="unix:path=$dir/bus" XDG_RUNTIME_DIR="$dir/run" XDG_CONFIG_HOME="$dir/config"
    XDG_DATA_HOME="$dir/data" XDG_CACHE_HOME="$dir/cache")
  env "${env[@]}" "${more[@]}" RUST_LOG=info,naga=warn,wgpu_core=warn,wgpu_hal=warn "$BIN/wprsd" >>"$WORK/wprsd-$name.log" 2>&1 &
  pids+=($!) && daemons[$name]=$!
  wait_for 30 env "${env[@]}" "$BIN/wprs-ctl" state || fail "daemon $name did not start"
}
device() {
  local dir=$WORK/$1 name=$1
  shift
  mkdir -p "$dir/run" && chmod 700 "$dir/run"
  printf '%s\n' "$@" | sed '/^$/d' >"$dir/env"
  pids+=("$(dbus-daemon --session --fork --address="unix:path=$dir/bus" --print-pid)")
  local env=(DBUS_SESSION_BUS_ADDRESS="unix:path=$dir/bus" XDG_RUNTIME_DIR="$dir/run" XDG_CONFIG_HOME="$dir/config"
    XDG_DATA_HOME="$dir/data" XDG_CACHE_HOME="$dir/cache")
  printf 'keyring password' | env "${env[@]}" gnome-keyring-daemon --unlock --components=secrets --foreground \
    >"$WORK/keyring-$name.log" 2>&1 & pids+=($!)
  wait_for 10 keyring "${env[0]}" || fail "the keyring of $name did not start"
  daemon "$name"
}
restart() {
  kill "${daemons[$1]}"
  wait "${daemons[$1]}" 2>/dev/null || true
  "${@:2}"
  daemon "$1"
}
a() { DBUS_SESSION_BUS_ADDRESS=unix:path=$WORK/a/bus "$BIN/wprs-ctl" "$@"; }
b() { DBUS_SESSION_BUS_ADDRESS=unix:path=$WORK/b/bus "$BIN/wprs-ctl" "$@"; }
c() { DBUS_SESSION_BUS_ADDRESS=unix:path=$WORK/c/bus "$BIN/wprs-ctl" "$@"; }
d() { DBUS_SESSION_BUS_ADDRESS=unix:path=$WORK/d/bus "$BIN/wprs-ctl" "$@"; }
finished() { [ "$("$1" job "$2" --json | jq_ "r['job']['state']")" = "done" ]; }
failed() { [ "$("$1" job "$2" --json | jq_ "r['job']['state']")" = "failed" ]; }
await() { wait_for "${3:-60}" finished "$1" "$2" || fail "job $2 of $1: $("$1" job "$2")"; }
repos() { [ "$(a remote ls --json | jq_ "len(r['repos'])")" = 1 ]; }
value() { "$1" server status --json | jq_ "r['account']$2"; }

pids+=("$(dbus-daemon --session --fork --address="unix:path=$WORK/bus" --print-pid)")
export DBUS_SESSION_BUS_ADDRESS=unix:path=$WORK/bus
pipewire >"$WORK/pipewire.log" 2>&1 & pids+=($!)
wait_for 10 test -S "$PIPEWIRE_RUNTIME_DIR/pipewire-0" || fail "PipeWire did not start"
wireplumber >"$WORK/wireplumber.log" 2>&1 & pids+=($!)
device a
device b LC_ALL= LC_MESSAGES=es_ES.UTF-8
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
a server approve "$(sed -n 's/^code = //p' <<<"$link")" --yes --wait >"$WORK/approve.out" 2>&1
{ grep -q "^platform = linux" "$WORK/approve.out" && grep -q "^software = wallpaper-rs/" "$WORK/approve.out"; } ||
  fail "approve did not show the pending device: $(cat "$WORK/approve.out")"
await b "$(tail -n 1 <<<"$link")"
[ "$(value b "['state']")" = needs_key ] || fail "second device: $(b server status --json)"
if a server approve BCDF-GHJK --yes 2>/dev/null; then fail "an unknown code was approved"; fi

sql() { python3 -c "import sqlite3, sys; print(*sqlite3.connect(sys.argv[1]).execute(sys.argv[2]).fetchone())" \
  "$WORK/server/db/wprs.sqlite" "$1"; }
write() { python3 -c "import sqlite3, sys; c = sqlite3.connect(sys.argv[1]); c.executescript(sys.argv[2]); c.commit()" \
  "$WORK/server/db/wprs.sqlite" "$1"; }
code() { "$1" server status --json | jq_ "((r['account']['pairing'] or {}).get('code') or '').replace(' ', '')"; }
pairing() { [ -n "$(code "$1")" ]; }
compare() {
  local joining=$1 holder=$2 from=$3 to=$4 job shown
  job=$("$joining" server join "$from")
  shown=$("$holder" server share "$to" 2>/dev/null | sed -n 's/^code = //p' | tr -d ' ')
  wait_for 30 pairing "$joining" || fail "no code on the joining device"
  echo "$job $shown $(code "$joining")"
}
real=$(sql "SELECT hex(enc_key) FROM devices WHERE number = 2")
write "UPDATE devices SET enc_key = randomblob(32) WHERE number = 2"
read -r job held joined < <(compare b a "Device 1" "Device 2")
[ "$held" != "$joined" ] || fail "a substituted key gave the same code"
if a server share "Device 2" --code "$joined" --wait 2>"$WORK/share.out"; then fail "a man in the middle got the key"; fi
grep -q "codes differ" "$WORK/share.out" || fail "code mismatch: $(cat "$WORK/share.out")"
wait_for 30 failed b "$job" || fail "the joining device kept waiting: $(b job "$job")"
[ "$(value b "['state']")" = needs_key ] || fail "the joining device took a key"
write "UPDATE devices SET enc_key = x'$real' WHERE number = 2"
read -r job held joined < <(compare b a "Device 1" "Device 2")
[ "$held" = "$joined" ] || fail "honest codes differ: $held $joined"
a server share "Device 2" --code "$joined" --wait >/dev/null
sleep 3
[ "$(value b "['state']")" = needs_key ] || fail "the joining device took the key before its own confirmation"
b server join "Device 1" --code "$held" --wait >/dev/null
await b "$job"
[ "$(value b "['state']")" = ready ] || fail "the key share: $(b server status --json)"
echo "link, approval and key share checks passed"

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
  "{\"epoch\": $epoch, \"changes\": [{\"collection\": \"playlists\", \"item\": \"$list\", \"base\": $seq, \"deleted\": true},
  {\"collection\": \"settings\", \"item\": \"transition\", \"base\": 0, \"deleted\": true}]}" >/dev/null
b server sync --wait >/dev/null
[ "$(b playlist ls --json | jq_ "len(r['playlists'])")" = 1 ] || fail "a tombstone without payload deleted a playlist"
a server sync --wait >/dev/null
sleep 8
before=$(sql "SELECT max(seq) FROM sync_records")
sleep 12
[ "$(sql "SELECT max(seq) FROM sync_records")" = "$before" ] || fail "the devices keep deleting a record of an old name"
echo "idle and tombstone checks passed"

site=$WORK/site
mkdir -p "$site/src"
cp "$ROOT/tests/fixtures/green-64x32.png" "$site/src/image.png"
cp "$ROOT/tests/fixtures/green-64x32.png" "$site/src/preview.png"
cat >"$site/src/manifest.json" <<'MANIFEST'
{"schema": "wallpaper-rs.wpak/1", "id": "org.example.lagoon", "version": "1.0.0", "title": "Lagoon",
 "license": "CC0-1.0", "kind": "image", "entry": "image.png", "preview": "preview.png", "tags": ["sea"]}
MANIFEST
(
  cd "$site"
  "$BIN/wprs-ctl" repo keygen root --no-password >/dev/null
  "$BIN/wprs-ctl" repo keygen online --no-password >/dev/null
  "$BIN/wprs-ctl" pack build src -o lagoon.wpak >/dev/null
  "$BIN/wprs-ctl" repo init repo --name Studio --root-key root.pub --online-key online.pub >/dev/null 2>&1
  "$BIN/wprs-ctl" repo sign-root repo --key root.key >/dev/null
  "$BIN/wprs-ctl" repo add repo lagoon.wpak >/dev/null
  "$BIN/wprs-ctl" repo publish repo --key online.key >/dev/null
)
anchor=$("$BIN/wprs-ctl" repo status "$site/repo" | sed -n 's/^anchor = //p')
sport=$(python3 -c "import socket; s = socket.socket(); s.bind(('127.0.0.1', 0)); print(s.getsockname()[1])")
python3 -m http.server --bind 127.0.0.1 --directory "$site/repo" "$sport" >"$WORK/http.log" 2>&1 & pids+=($!)
wait_for 10 python3 -c "import urllib.request; urllib.request.urlopen('http://127.0.0.1:$sport/entry/latest')" ||
  fail "the repository of the studio did not start"
a remote add "http://127.0.0.1:$sport/$anchor" --wait >/dev/null
a server sync --wait >/dev/null
b server sync --wait >/dev/null
offers() { b server status --json | jq_ "[o['id'] for o in r['account']['offers'] if o['name'] == 'Studio']"; }
offer=$(offers | tr -d "[]' ")
[ -n "$offer" ] || fail "the repository was not offered: $(b server status)"
[ "$(b remote ls --json | jq_ "len(r['repos'])")" = 1 ] || fail "a synced repository was added without asking"
b server repo "$offer" --wait >/dev/null
[ "$(b remote ls --json | jq_ "sorted(x['name'] for x in r['repos'])")" = "['Example', 'Studio']" ] ||
  fail "the confirmed repository: $(b remote ls)"
b server sync --wait >/dev/null
[ "$(offers)" = "[]" ] || fail "the added repository is still offered"
echo "repository checks passed"

own=$WORK/own
mkdir -p "$own"
cp "$ROOT/tests/fixtures/green-64x32.png" "$own/meadow.png"
a lib add "$own" --wait >/dev/null
a server files share "$own" --wait >/dev/null
a server sync --wait >/dev/null
b server sync --wait >/dev/null
file=$(b server files ls --json | jq_ "[f['id'] for f in r['files'] if f['name'] == 'meadow.png' and f['item'] is None]" | tr -d "[]' ")
[ -n "$file" ] || fail "the private file is not listed: $(b server files ls)"
b server files fetch "$file" --wait >/dev/null
cmp -s "$own/meadow.png" "$WORK/b/data/wallpaper-rs/files/meadow.png" || fail "the fetched file differs"
b server sync --wait >/dev/null
[ "$(b server files ls --json | jq_ "[f['item'] is not None for f in r['files']]")" = "[True]" ] ||
  fail "the fetched file is not linked to its item: $(b server files ls)"
python3 - "$WORK/server/private" "$own/meadow.png" <<'CHECK' || fail "the server holds a readable private file"
import pathlib, sys
stored = [p for p in pathlib.Path(sys.argv[1]).rglob("*") if p.is_file()]
plain = pathlib.Path(sys.argv[2]).read_bytes()
assert len(stored) == 1 and stored[0].stat().st_size == len(plain) + 16, stored
assert plain[:8] not in stored[0].read_bytes() and plain[-16:] not in stored[0].read_bytes()
CHECK
echo "private file checks passed"

b server pause
a playlist query "Quiet evenings" "tag:evening-glow" >/dev/null
a server sync --wait >/dev/null
meadow=$(b lib search meadow --json | jq_ "r['items'][0]['uid']")
b playlist add "Quiet evenings" "$meadow" >/dev/null
b server sync --wait >/dev/null
b server sync --wait >/dev/null
b server resume
a server sync --wait >/dev/null
lists() { "$1" playlist ls --json | jq_ "sorted((p['name'], p['query']) for p in r['playlists'])"; }
expected="[('Quiet evenings', None), ('Quiet evenings (conflicto)', 'tag:evening-glow')]"
[ "$(lists a)" = "$expected" ] || fail "the conflict copy on the first device: $(lists a)"
[ "$(lists b)" = "$expected" ] || fail "the conflict copy on the second device: $(lists b)"
[ "$(a playlist show "Quiet evenings" --json | jq_ "r['total']")" = 2 ] || fail "the winning side lost an entry"
echo "conflict checks passed"

for dev in a b; do
  "$dev" provider add nas webdav url=http://127.0.0.1:9/dav user=reader refresh=0s --no-secret >/dev/null
done
printf 'swordfish-on-the-quay\n' | a provider secret nas --secret-stdin
a server secret nas
a server sync --wait >/dev/null
b server sync --wait >/dev/null
[ "$(b provider ls --json | jq_ "[p['has_secret'] for p in r['providers'] if p['id'] == 'nas']")" = "[True]" ] ||
  fail "the provider secret did not arrive: $(b provider ls)"
[ "$(value b "['secrets']")" = "['nas']" ] || fail "shared secrets: $(b server status)"
b provider set nas user someone-else >/dev/null
b server sync --wait >/dev/null
[ "$(b provider ls --json | jq_ "[p['has_secret'] for p in r['providers'] if p['id'] == 'nas']")" = "[False]" ] ||
  fail "a secret reached a provider with another user"
echo "secret checks passed"

phrase="copper lantern drifts over a quiet harbor"
printf '%s\n' "$phrase" | a server escrow --wait >/dev/null
[ "$(value a "['recovery']")" = current ] || fail "escrow: $(a server status)"
device c
c server add "$URL" --wait >/dev/null
link=$(c server link --name Spare)
api -d "{\"user_code\": \"$(sed -n 's/^code = //p' <<<"$link")\", \"approve\": true}" "$URL/api/v1/auth/device/approve" >/dev/null
await c "$(tail -n 1 <<<"$link")"
[ "$(value c "['state']")" = needs_key ] || fail "third device: $(c server status --json)"
if printf 'not the right passphrase at all\n' | c server restore --wait 2>/dev/null; then fail "a wrong passphrase restored the key"; fi
printf '%s\n' "$phrase" | c server restore --wait >/dev/null
c server sync --wait >/dev/null
[ "$(lists c)" = "$expected" ] || fail "the restored device: $(lists c)"
echo "escrow checks passed"

device d
d server add "$URL" --wait >/dev/null
link=$(d server link --name Tablet)
api -d "{\"user_code\": \"$(sed -n 's/^code = //p' <<<"$link")\", \"approve\": true}" "$URL/api/v1/auth/device/approve" >/dev/null
await d "$(tail -n 1 <<<"$link")"
read -r d_id d_name < <(sql "SELECT id, 'Device ' || number FROM devices ORDER BY number DESC LIMIT 1")
read -r a_id a_key < <(sql "SELECT id, hex(enc_key) FROM devices WHERE number = 1")
c_id=$(sql "SELECT id FROM devices WHERE number = 4")
write "UPDATE devices SET enc_key = (SELECT enc_key FROM devices WHERE number = 4) WHERE number = 1"
python3 - "$WORK/server/db/wprs.sqlite" "$a_id" "$c_id" "$d_id" "$WORK/relay.stop" <<'RELAY' & relay=$!
import os, sqlite3, sys, time
db, a, c, d, stop = sys.argv[1:]
while not os.path.exists(stop):
    with sqlite3.connect(db, timeout=10) as conn:
        conn.execute("UPDATE mailbox SET to_device = ? WHERE to_device = ? AND from_device = ?", (c, a, d))
        conn.execute("UPDATE mailbox SET from_device = ? WHERE from_device = ? AND to_device = ?", (a, c, d))
    time.sleep(0.2)
RELAY
job=$(d server join "Device 1")
shown=$(c server share "$d_name" 2>/dev/null | sed -n 's/^code = //p' | tr -d ' ')
wait_for 30 pairing d || fail "no code on the device that joins through the server"
[ "$shown" = "$(code d)" ] || fail "the server did not stand between the devices"
c server share "$d_name" --code "$shown" --wait >/dev/null
sleep 4
[ "$(value d "['state']")" = needs_key ] || fail "a key from the server's device was taken without a confirmation here"
wrong=$(printf '%06d' $(((10#$shown + 1) % 1000000)))
if d server join "Device 1" --code "$wrong" --wait 2>"$WORK/join.out"; then fail "a wrong code took the key"; fi
grep -q "codes differ" "$WORK/join.out" || fail "the wrong code: $(cat "$WORK/join.out")"
[ "$(value d "['state']")" = needs_key ] || fail "the device that joined through the server took a key"
touch "$WORK/relay.stop" && wait "$relay"
write "UPDATE devices SET enc_key = x'$a_key' WHERE number = 1; DELETE FROM mailbox"
read -r job held joined < <(compare d a "Device 1" "$d_name")
a server share "$d_name" --code "$joined" --wait >/dev/null
d server join "Device 1" --code "$held" --wait >/dev/null
await d "$job"
[ "$(value d "['state']")" = ready ] || fail "the fourth device: $(d server status --json)"
echo "mutual comparison checks passed"

a server sync --wait >/dev/null
b server sync --wait >/dev/null
a server sync --wait >/dev/null
write "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 100)
  INSERT INTO mailbox (user_id, to_device, from_device, kind, body, created_at, expires_at)
  SELECT user_id, id, id, 'ping', x'00', 0, 9999999999999 FROM devices, n WHERE id = '$d_id'"
a server rotate --wait --json | jq_ "r['output']['shared']" >"$WORK/shared.out"
{ grep -q "Device 2" "$WORK/shared.out" && ! grep -q "$d_name" "$WORK/shared.out"; } ||
  fail "the new key was not handed over: $(cat "$WORK/shared.out")"
b server sync --wait >/dev/null || fail "the second device did not take the new key: $(b server status)"
write "DELETE FROM mailbox WHERE kind = 'ping'"
a server sync --wait >/dev/null
d server sync --wait >/dev/null || fail "the new key did not reach the device whose mailbox was full: $(d server status)"
[ "$(sql "SELECT COUNT(*) FROM sync_records WHERE epoch < (SELECT epoch FROM sync_heads)")" = 0 ] ||
  fail "records of the old key epoch stayed"
a server sync --wait >/dev/null
[ "$(value a "['recovery']")" = current ] || fail "the escrow was not sealed again after the rotation"
d server pause
b_id=$(value b "['devices'][1]['id']")
write "CREATE TABLE kept_devices AS SELECT * FROM devices WHERE id = '$b_id';
  CREATE TABLE kept_tokens AS SELECT * FROM tokens WHERE device_id = '$b_id'"
read -r epoch old_key < <(sql "SELECT epoch, key_id FROM sync_heads")
curl -fsS -b "$jar" -X DELETE "$URL/api/v1/me/devices/$b_id" >/dev/null
a server sync --wait >/dev/null
[ "$(sql "SELECT epoch FROM sync_heads")" = $((epoch + 1)) ] || fail "revoking a device did not replace the key"
[ "$(value a "['recovery']")" = stale ] || fail "the escrow was sealed again after a revocation: $(a server status)"
write "INSERT INTO devices SELECT * FROM kept_devices; INSERT INTO tokens SELECT * FROM kept_tokens;
  DROP TABLE kept_devices; DROP TABLE kept_tokens"
a playlist new "After the revocation" >/dev/null
a server sync --wait >/dev/null
if b server sync --wait 2>"$WORK/revoked.out"; then fail "a revoked device synced after the rotation"; fi
[ "$(value b "['state']")" = needs_key ] || fail "the revoked device: $(b server status --json)"
if b playlist ls | grep -q "After the revocation"; then fail "a revoked device read a record after the rotation"; fi
write "DELETE FROM mailbox WHERE to_device = '$d_id'; DELETE FROM sync_records WHERE epoch > $epoch;
  UPDATE sync_heads SET epoch = $epoch, key_id = '$old_key'"
pin() {
  python3 - "$WORK/b/data/wallpaper-rs/sync.json" "$d_id" "$(sql "SELECT hex(enc_key) FROM devices WHERE id = '$d_id'")" <<'PIN'
import base64, json, sys
path, device, key = sys.argv[1:]
saved = json.load(open(path))
saved["pinned"][device] = base64.b64encode(bytes.fromhex(key)).decode()
json.dump(saved, open(path, "w"))
PIN
}
restart b pin
b server sync --wait >/dev/null || fail "the revoked device did not sync again: $(b server status)"
b playlist new "From the revoked device" >/dev/null
b server rotate --wait --json | jq_ "r['output']['shared']" | grep -q "$d_name" || fail "the revoked device sent no key"
b server sync --wait >/dev/null
d server resume
if d server sync --wait 2>/dev/null; then fail "a device that missed the revocation took the revoked device's key"; fi
[ "$(value d "['state']")" = needs_key ] || fail "the lagging device: $(d server status --json)"
if d playlist ls | grep -q "From the revoked device"; then fail "the lagging device read the revoked device"; fi
echo "rotation and revocation checks passed"

b server reset --yes --wait >/dev/null
[ "$(value b "['state']")" = ready ] || fail "reset: $(b server status --json)"
if a server sync --wait 2>/dev/null; then fail "a device with the old key synced after a reset"; fi
[ "$(value a "['state']")" = needs_key ] || fail "after the reset: $(a server status --json)"
read -r job held joined < <(compare a b "Device 2" "Device 1")
b server share "Device 1" --code "$joined" --wait >/dev/null
a server join "Device 2" --code "$held" --wait >/dev/null
await a "$job"
a server sync --wait >/dev/null
b server sync --wait >/dev/null
a server sync --wait >/dev/null
names() { "$1" playlist ls --json | jq_ "sorted(p['name'] for p in r['playlists'])"; }
expected="['After the revocation', 'From the revoked device', 'Quiet evenings', 'Quiet evenings (conflicto)']"
[ "$(names a)" = "$expected" ] || fail "the playlists after the reset on the first device: $(names a)"
[ "$(names b)" = "$expected" ] || fail "the playlists after the reset on the second device: $(names b)"
echo "reset checks passed"

mkdir -p "$WORK/phone" && mkfifo "$WORK/phone.in" "$WORK/phone.out"
"$BIN/phone" "$WORK/phone" <"$WORK/phone.in" >"$WORK/phone.out" 2>>"$WORK/phone.log" & pids+=($!)
exec 7>"$WORK/phone.in" 8<"$WORK/phone.out"
send() { printf '%s\n' "$*" >&7; }
reply() {
  local line
  IFS= read -r -t 900 line <&8 || fail "the phone did not answer"
  [[ $line != '{"error":'* ]] || fail "the phone: $line"
  printf '%s\n' "$line"
}
ask() { send "$@" && reply; }
field() { jq_ "r$2" <<<"$1"; }
phone() { ask 'account {"op": "status"}' | jq_ "r$1"; }
named() { ask "library {\"op\": \"$1\"$2}" | jq_ "sorted(${3:-x['playlist']['name'] for x in r})"; }

a config transition.duration_ms 900 >/dev/null
a config playback.max_fps 60 >/dev/null
a lib tag "$(a lib search meadow --json | jq_ "r['items'][0]['uid']")" --add from-the-desk >/dev/null
a server sync --wait >/dev/null
green=$(ask "import $ROOT/tests/fixtures/green-64x32.png" | jq_ "r")
mix=$(ask 'library {"op": "create", "name": "Phone mix"}' | jq_ "r")
ask "library {\"op\": \"add\", \"playlist\": \"$mix\", \"item\": \"$green\"}" >/dev/null
[ "$(phone "['state']")" = none ] || fail "a new phone: $(phone "")"
ask "account {\"op\": \"add\", \"url\": \"$URL\"}" >/dev/null
started=$(ask 'account {"op": "link", "label": "Pocket", "plaintext": false}')
send 'account {"op": "wait"}'
a server approve "$(field "$started" "['code']")" --yes --wait >"$WORK/phone-approve.out" 2>&1
grep -q "^platform = android" "$WORK/phone-approve.out" || fail "the phone link: $(cat "$WORK/phone-approve.out")"
linked=$(reply)
pocket=$(field "$linked" "['device']")
[ "$(field "$linked" "['needs_key']")/$(phone "['state']")" = True/needs_key ] || fail "the linked phone: $linked"
send 'account {"op": "join", "device": "Device 1"}'
held=$(a server share "$pocket" 2>/dev/null | sed -n 's/^code = //p' | tr -d ' ')
joined=$(reply | jq_ "r['code'].replace(' ', '')")
[ -n "$held" ] || fail "the desktop showed no code for the phone"
[ "$held" = "$joined" ] || fail "the phone and the desktop show different codes: $held $joined"
a server share "$pocket" --code "$joined" --wait >/dev/null
[ "$(ask "account {\"op\": \"confirm\", \"code\": \"$held\"}" | jq_ "r['from']")" = "Device 1" ] || fail "the phone join"
[ "$(phone "['state']")" = ready ] || fail "the phone after the join: $(phone "")"
ask 'account {"op": "sync", "conflict": "%1 (conflict)"}' >/dev/null
expected="['After the revocation', 'From the revoked device', 'Phone mix', 'Quiet evenings', 'Quiet evenings (conflicto)']"
[ "$(named playlists "")" = "$expected" ] || fail "the playlists on the phone: $(named playlists "")"
[ "$(named items ', "query": "tag:from-the-desk"' "x['title'] for x in r")" = "['meadow']" ] ||
  fail "the desktop's title and tag did not reach the phone: $(ask 'library {"op": "items", "query": ""}')"
[ "$(ask "setting transition.duration_ms")/$(ask "setting playback.max_fps")" = 900/30 ] ||
  fail "settings on the phone: $(ask "setting transition.duration_ms") $(ask "setting playback.max_fps")"
ask "library {\"op\": \"favorite\", \"uid\": \"$green\", \"favorite\": true}" >/dev/null
ask 'account {"op": "sync", "conflict": "%1 (conflict)"}' >/dev/null
a server sync --wait >/dev/null
[ "$(a playlist show "Phone mix" --json | jq_ "r['total']")" = 1 ] || fail "the phone's playlist on the desktop: $(a playlist ls)"
[ "$(a lib search fav:yes --json | jq_ "[i['title'] for i in r['items']]")" = "['meadow']" ] ||
  fail "the phone's favourite on the desktop: $(a lib search fav:yes)"
[ "$(a config playback.max_fps)" = "playback.max_fps = 60" ] || fail "the phone changed a desktop-only setting"
[ "$(value a "['devices'][-1]['label']")" = Pocket ] || fail "the phone's name on the desktop: $(a server status)"
echo "phone link, join and sync checks passed"

a server rotate --wait --json | jq_ "r['output']['shared']" | grep -q "$pocket" || fail "the new key did not go to the phone"
ask 'account {"op": "sync", "conflict": "%1 (conflict)"}' >/dev/null
[ "$(phone "['state']")" = ready ] || fail "the phone after a rotation: $(phone "")"
a playlist new "After the phone" >/dev/null
a server sync --wait >/dev/null
ask 'account {"op": "sync", "conflict": "%1 (conflict)"}' >/dev/null
named playlists "" | grep -q "After the phone" || fail "the phone did not read a record after the rotation"
echo "phone rotation checks passed"

b server remove --wipe
a server sync --wait >/dev/null
ask 'account {"op": "sync", "conflict": "%1 (conflict)"}' >/dev/null
b server add "$URL" --wait >/dev/null
link=$(b server link --name Laptop)
code=$(sed -n 's/^code = //p' <<<"$link")
[ "$(ask "account {\"op\": \"preview\", \"code\": \"$code\"}" | jq_ "r['platform']")" = linux ] || fail "the phone preview"
ask "account {\"op\": \"approve\", \"code\": \"$code\", \"approve\": true}" >/dev/null
await b "$(tail -n 1 <<<"$link")"
laptop=$(value b "['device']")
job=$(b server join "$pocket")
shown=$(ask "account {\"op\": \"share\", \"device\": \"$laptop\"}" | jq_ "r['code'].replace(' ', '')")
wait_for 30 pairing b || fail "no code on the laptop that joins from the phone"
[ "$shown" = "$(code b)" ] || fail "the phone and the laptop show different codes"
ask "account {\"op\": \"confirm\", \"code\": \"$(code b)\"}" >/dev/null
b server join "$pocket" --code "$shown" --wait >/dev/null
await b "$job"
b server sync --wait >/dev/null
b playlist ls | grep -q "Phone mix" || fail "the laptop that joined from the phone: $(b playlist ls)"
ask 'account {"op": "pause", "paused": true}' >/dev/null
[ "$(phone "['state']")" = paused ] || fail "the paused phone: $(phone "")"
if grep -r -q -s -a -e 'wprs_d_' "$WORK/phone/sync.json" "$WORK/phone.log"; then fail "a phone token reached a file or a log"; fi
ask 'account {"op": "remove"}' >/dev/null
[ "$(phone "['state']")" = none ] || fail "the removed phone: $(phone "")"
[ "$(api "$URL/api/v1/me/devices" | jq_ "[d['platform'] for d in r['items']].count('android')")" = 0 ] ||
  fail "the server kept the removed phone"
exec 7>&- 8<&-
echo "phone holder, approval and removal checks passed"

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
  if grep -q -a -e evening-glow -e 'Quiet evenings' -e Laptop -e swordfish "$f"; then fail "$f holds plain text"; fi
done
if grep -r -q -s -a -e swordfish "$WORK"/a/data "$WORK"/a/config "$WORK"/b/data "$WORK"/b/config "$WORK"/*.log; then
  fail "a provider secret reached a file or a log"
fi
if grep -r -q -s -a -e 'wprs_d_' "$WORK/a/data" "$WORK/a/config" "$WORK/b/data" "$WORK/c/data" "$WORK/d/data" "$WORK"/wprsd-*.log; then
  fail "a token reached a file or a log"
fi
a server remove --wipe
[ "$(value a "")" = None ] || fail "the server was kept"
status=0
echo "server sync test passed"
