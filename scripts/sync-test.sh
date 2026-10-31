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
  for _ in $(seq 50); do
    alive=0
    for p in "${pids[@]}"; do kill -0 "$p" 2>/dev/null && alive=1; done
    [ "$alive" = 0 ] && break
    sleep 0.1
  done
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
code=$(sed -n 's/^code = //p' <<<"$link")
if printf 'not the password\n' | a server approve "$code" --yes --wait >/dev/null 2>&1; then
  fail "a device was approved with a wrong password"
fi
printf 'correct-horse-battery\n' | a server approve "$code" --yes --wait >"$WORK/approve.out" 2>&1
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
source "$(dirname "$0")"/sync-library.sh
source "$(dirname "$0")"/sync-files.sh
source "$(dirname "$0")"/sync-keys.sh
source "$(dirname "$0")"/sync-phone.sh

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
if grep -r -q -s -a -e 'wprs_d_' -e correct-horse-battery "$WORK/a/data" "$WORK/a/config" "$WORK/b/data" "$WORK/c/data" "$WORK/d/data" "$WORK"/wprsd-*.log; then
  fail "a token or the account password reached a file or a log"
fi
a server remove --wipe
[ "$(value a "")" = None ] || fail "the server was kept"
status=0
echo "server sync test passed"
