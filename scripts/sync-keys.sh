#!/usr/bin/env bash
set -euo pipefail
declare jar
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
if printf '%s\n' "$phrase" | a server escrow --wait 2>"$WORK/same.out"; then fail "the passphrase from before the revocation was set again"; fi
grep -q "choose a new, different passphrase" "$WORK/same.out" || fail "reusing the passphrase: $(cat "$WORK/same.out")"
a server status | grep -q "^recovery = stale: a device that may know the passphrase was revoked" ||
  fail "the stale escrow is not explained: $(a server status)"
printf 'a new phrase after the revocation of the laptop\n' | a server escrow --wait >/dev/null
[ "$(value a "['recovery']")" = current ] || fail "the new passphrase: $(a server status)"
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
