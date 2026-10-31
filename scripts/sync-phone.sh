#!/usr/bin/env bash
set -euo pipefail
declare own
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
a edit set "$own/meadow.png" ev=0.5 >/dev/null
a server sync --wait >/dev/null
green=$(ask "import $ROOT/tests/fixtures/green-64x32.png" | jq_ "r")
ask "depth {\"op\": \"fit\", \"uids\": [\"$green\"], \"fit\": \"center\"}" >/dev/null
ask "depth {\"op\": \"parallax\", \"uid\": \"$green\", \"on\": true}" >/dev/null
mix=$(ask 'library {"op": "create", "name": "Phone mix"}' | jq_ "r")
ask "library {\"op\": \"add\", \"playlist\": \"$mix\", \"items\": [\"$green\"]}" >/dev/null
[ "$(phone "['state']")" = none ] || fail "a new phone: $(phone "")"
ask "account {\"op\": \"add\", \"url\": \"$URL\"}" >/dev/null
started=$(ask 'account {"op": "link", "label": "Pocket", "plaintext": false}')
send 'account {"op": "wait"}'
printf 'correct-horse-battery\n' | a server approve "$(field "$started" "['code']")" --yes --wait >"$WORK/phone-approve.out" 2>&1
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
phoned=$(find "$WORK/phone" -name '*.wprs.json' | head -n 1)
[ "$(jq_ "(r['ops'][0]['ev'], r['fit'], r['parallax']['enabled'])" <"$phoned")" = "(0.5, 'center', True)" ] ||
  fail "edits made apart before the first sync did not meet on the phone: $(cat "$phoned")"
ask "library {\"op\": \"favorite\", \"uids\": [\"$green\"], \"favorite\": true}" >/dev/null
ask 'account {"op": "sync", "conflict": "%1 (conflict)"}' >/dev/null
a server sync --wait >/dev/null
[ "$(a edit show "$own/meadow.png" | jq_ "(r['ops'][0]['ev'], r['fit'], r['parallax']['source'])")" = "(0.5, 'center', 'auto')" ] ||
  fail "the phone's fit and parallax did not reach the desktop next to its own edit: $(a edit show "$own/meadow.png")"
a edit crop "$own/meadow.png" 0.25,0,0.5,1 >/dev/null
a server sync --wait >/dev/null
ask 'account {"op": "sync", "conflict": "%1 (conflict)"}' >/dev/null
framed=$(ask "depth {\"op\": \"item\", \"uid\": \"$green\"}" | jq_ "(r['base'], r['crop'], r['fit'])")
[ "$framed" = "([0.25, 0.0, 0.5, 1.0], None, 'center')" ] || fail "the desktop's crop did not reach the phone: $framed"
ask "depth {\"op\": \"frame\", \"uid\": \"$green\", \"pan\": [0.1, 0], \"zoom\": 2, \"screen\": [1080, 2400], \"done\": true}" >/dev/null
ask 'account {"op": "sync", "conflict": "%1 (conflict)"}' >/dev/null
a server sync --wait >/dev/null
[ "$(a edit show "$own/meadow.png" | jq_ "(r['geometry']['crop'], sorted(r['outputs']))")" = "([0.25, 0.0, 0.5, 1.0], ['Android'])" ] ||
  fail "the phone's framing changed the desktop's crop: $(a edit show "$own/meadow.png")"
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
ask "account {\"op\": \"approve\", \"code\": \"$code\", \"approve\": true, \"password\": \"correct-horse-battery\"}" >/dev/null
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
if grep -r -q -s -a -e 'wprs_d_' -e correct-horse-battery "$WORK/phone/sync.json" "$WORK/phone.log"; then
  fail "a phone token or the account password reached a file or a log"
fi
ask 'account {"op": "remove"}' >/dev/null
[ "$(phone "['state']")" = none ] || fail "the removed phone: $(phone "")"
[ "$(api "$URL/api/v1/me/devices" | jq_ "[d['platform'] for d in r['items']].count('android')")" = 0 ] ||
  fail "the server kept the removed phone"
exec 7>&- 8<&-
echo "phone holder, approval and removal checks passed"
