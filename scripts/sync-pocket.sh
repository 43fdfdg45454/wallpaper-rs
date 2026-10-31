#!/usr/bin/env bash
set -euo pipefail
declare own
sync_phone() { ask 'account {"op": "sync", "conflict": "%1 (conflict)"}'; }
[ "$(phone "['secrets']")" = "['nas']" ] || fail "the phone does not list the shared secret: $(phone "")"
short=$(send 'account {"op": "escrow", "passphrase": "too short"}' && IFS= read -r -t 900 line <&8 && echo "$line")
grep -q '"id":"android.phrase_short"' <<<"$short" || fail "a short passphrase was taken: $short"
pocket_phrase="seven lanterns wander past a sleeping lighthouse"
ask "account {\"op\": \"escrow\", \"passphrase\": \"$pocket_phrase\"}" >/dev/null
[ "$(phone "['recovery']")" = current ] || fail "the phone's recovery escrow: $(phone "")"
a server sync --wait >/dev/null
[ "$(value a "['recovery']")" = current ] || fail "the desktop does not see the phone's escrow: $(a server status)"
ask 'account {"op": "rotate"}' | grep -q '"epoch":' || fail "the phone did not replace the sync key"
[ "$(phone "['recovery']")" = current ] || fail "the phone's rotation left its recovery escrow behind: $(phone "")"
a server sync --wait >/dev/null
[ "$(value a "['recovery']")" = current ] || fail "the desktop sees a stale escrow after the phone's rotation: $(a server status)"
if grep -q -s -a -e swordfish "$WORK/phone/sync.json" "$WORK/phone.log"; then
  fail "a provider secret reached the sync state or the log of the phone"
fi

studio=$(ask 'account {"op": "repos"}' | jq_ "[o['id'] for o in r['offers'] if o['name'] == 'Studio'][0]")
ask "account {\"op\": \"offer\", \"repo\": \"$studio\", \"add\": true}" >/dev/null
repo_id=$(ask 'account {"op": "browse", "query": "lagoon"}' | jq_ "r[0]['repo_id']")
item=$(ask "account {\"op\": \"install\", \"repo_id\": \"$repo_id\", \"id\": \"org.example.lagoon\"}" | jq_ "r['item']")
[ "$(ask 'account {"op": "browse", "query": "lagoon"}' | jq_ "r[0]['installed']")" = True ] || fail "lagoon was not installed"
[ "$(ask "library {\"op\": \"item\", \"uid\": \"$item\"}" | jq_ "r['kind']")" = package ] || fail "the installed package"
[ "$(ask 'account {"op": "repos"}' | jq_ "sorted(x['name'] for x in r['repos'])")" = "['Studio']" ] || fail "the phone repositories"
sync_phone >/dev/null
a server sync --wait >/dev/null
a server status --json | jq_ "[o['name'] for o in r['account']['offers']]" | grep -q Studio &&
  fail "the repository confirmed on the phone came back to the desktop as an offer"
echo "phone repository checks passed"

cp "$ROOT/tests/fixtures/bands-640x180.png" "$own/dunes.png"
a lib scan --wait >/dev/null
a server files share "$own" --wait >/dev/null
a server sync --wait >/dev/null
sync_phone >/dev/null
files=$(ask 'account {"op": "files"}')
dunes=$(jq_ "[f['id'] for f in r['files'] if f['name'] == 'dunes.png' and f['item'] is None]" <<<"$files" | tr -d "[]' ")
[ -n "$dunes" ] || fail "the phone does not list the new private file: $files; the desktop: $(a server files ls)"
fetched=$(ask "account {\"op\": \"fetch\", \"id\": \"$dunes\"}" | jq_ "r['item']")
path=$(ask "library {\"op\": \"item\", \"uid\": \"$fetched\"}" | jq_ "r['path']")
cmp -s "$own/dunes.png" "$path" || fail "the file fetched by the phone differs"
sync_phone >/dev/null
[ "$(ask 'account {"op": "files"}' | jq_ "[f['item'] == '$fetched' for f in r['files'] if f['name'] == 'dunes.png']")" = "[True]" ] ||
  fail "the fetched file is not linked to its item on the phone: $(ask 'account {"op": "files"}')"
mine=$(ask "import $ROOT/tests/fixtures/quadrants-64x32.webm" | jq_ "r")
ask "account {\"op\": \"share_files\", \"targets\": [\"$mine\"], \"share\": true}" >/dev/null
synced=$(sync_phone)
[ "$(jq_ "r['uploaded']" <<<"$synced")" = 0 ] || fail "the second round uploaded again: $synced"
a server sync --wait >/dev/null
listed=$(a server files ls --json)
sent=$(jq_ "[f['id'] for f in r['files'] if f['name'].startswith('quadrants') and f['item'] is None]" <<<"$listed" | tr -d "[]' ")
[ -n "$sent" ] || fail "the phone's private file did not reach the desktop: $listed; the phone: $(ask 'account {"op": "files"}')"
a server files fetch "$sent" --wait >/dev/null
cmp -s "$ROOT/tests/fixtures/quadrants-64x32.webm" "$(find "$WORK/a/data/wallpaper-rs/files" -name 'quadrants*' | head -n 1)" ||
  fail "the phone's private file reached the desktop changed"
ask "account {\"op\": \"share_files\", \"targets\": [\"$mine\"], \"share\": false}" >/dev/null
sync_phone >/dev/null
sync_phone >/dev/null
a server sync --wait >/dev/null
[ "$(a server files ls --json | jq_ "[f['name'] for f in r['files'] if f['name'].startswith('quadrants') and f['item'] is None]")" = "[]" ] ||
  fail "a file the phone stopped sharing is still offered: $(a server files ls)"
echo "phone private file checks passed"

send 'account {"op": "listen", "token": 1}'
a playlist new "Heard at once" >/dev/null
a server sync --wait >/dev/null
[ "$(reply | jq_ "r['changed']")" = True ] || fail "the phone did not hear the change"
sync_phone >/dev/null
named playlists "" | grep -q "Heard at once" || fail "the change the phone heard did not arrive"
send 'account {"op": "listen", "token": 2}'
send 'account {"op": "hush", "token": 2}'
answers=$(reply && reply)
grep -q '"changed":false' <<<"$answers" || fail "hushing the phone did not end its listening: $answers"
send 'account {"op": "listen", "token": 4}'
[ "$(ask 'account {"op": "hush", "token": 3}')" = null ] || fail "the phone did not take an old hush"
a playlist new "Heard after an old hush" >/dev/null
a server sync --wait >/dev/null
[ "$(reply | jq_ "r['changed']")" = True ] || fail "the hush of an older listener silenced the phone"
sync_phone >/dev/null
echo "phone event checks passed"

mkdir -p "$WORK/spare" && mkfifo "$WORK/spare.in" "$WORK/spare.out"
"$BIN/phone" "$WORK/spare" <"$WORK/spare.in" >"$WORK/spare.out" 2>>"$WORK/spare.log" & pids+=($!)
exec 5>"$WORK/spare.in" 6<"$WORK/spare.out"
spare() { printf '%s\n' "$*" >&5 && IFS= read -r -t 900 line <&6 && printf '%s\n' "$line"; }
spare "account {\"op\": \"add\", \"url\": \"$URL\"}" >/dev/null
started=$(spare 'account {"op": "link", "label": "Spare", "plaintext": false}')
printf '%s\n' 'account {"op": "wait"}' >&5
printf 'correct-horse-battery\n' | a server approve "$(field "$started" "['code']")" --yes --wait >/dev/null 2>&1
IFS= read -r -t 900 line <&6 || fail "the spare phone was not linked"
wrong=$(spare 'account {"op": "restore", "passphrase": "not the passphrase of this account"}')
grep -q '"error":' <<<"$wrong" || fail "a wrong passphrase restored the key on the spare phone: $wrong"
spare "account {\"op\": \"restore\", \"passphrase\": \"$pocket_phrase\"}" >/dev/null
spare 'account {"op": "sync", "conflict": "%1 (conflict)"}' >/dev/null
spare 'library {"op": "playlists"}' | grep -q "Phone mix" || fail "the restored spare phone did not sync"
spare 'account {"op": "remove"}' >/dev/null
exec 5>&- 6<&-
echo "phone recovery checks passed"
