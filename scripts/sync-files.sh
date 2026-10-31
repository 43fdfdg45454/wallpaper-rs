#!/usr/bin/env bash
set -euo pipefail
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
cp "$ROOT/tests/fixtures/bands-640x180.png" "$own/lost.png"
a lib add "$own" --wait >/dev/null
rm "$own/lost.png"
if a server files share "$own" --wait >"$WORK/share-files.out" 2>&1; then fail "a missing file was uploaded"; fi
grep -q "1 private files were not uploaded, the others were: lost" "$WORK/share-files.out" ||
  fail "the missing file was not reported: $(cat "$WORK/share-files.out")"
a lib rm "$(a lib search lost --json | jq_ "r['items'][0]['uid']")" >/dev/null
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
