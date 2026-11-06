#!/usr/bin/env bash
set -euo pipefail
declare daemon
js() { python3 -c "import json, sys; r = json.load(sys.stdin); print(eval(sys.argv[1]))" "$1"; }
png() {
  python3 - "$1" "$2" <<'PNG'
import struct, sys, zlib
def chunk(kind, data):
    return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))
pixels = (b"\0" + bytes.fromhex(sys.argv[2]) * 64) * 32
with open(sys.argv[1], "wb") as out:
    out.write(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", 64, 32, 8, 2, 0, 0, 0)))
    out.write(chunk(b"IDAT", zlib.compress(pixels)) + chunk(b"IEND", b""))
PNG
}
rot=$WORK/rotation
mkdir -p "$rot"
png "$rot/a-red.png" ff0000
png "$rot/b-blue.png" 0000ff
cp "$ROOT/tests/fixtures/green-64x32.png" "$rot/c-green.png"
rot_source=$(ctl lib add "$rot" --wait | sed -n 1p)
read -r red blue green < <(ctl lib search "sort:title" --json | js "' '.join(i['uid'] for i in r['items'])")
ctl playlist new Colors >/dev/null
ctl playlist add Colors "$red" "$blue" "$green"
[ "$(ctl playlist show Colors --json | js "r['total'], r['playlist']['query']")" = "(3, None)" ] ||
  fail "manual playlist: $(ctl playlist show Colors --json)"
read -r _ _ _ s8 < <(ctl stream TEST-8 160 90)
shown="s['outputs']['TEST-8']['item']"
color() { "$BIN/probe" "$s8" 3 | grep "center bgra \[$1\]" >/dev/null; }
ctl playlist start Colors -o TEST-8
expect "$shown" "$red"
expect "s['outputs']['TEST-8']['playlist']['name'], s['outputs']['TEST-8']['playlist']['count']" "('Colors', 3)"
retry 10 color "0, 0, 255, 255" || fail "red is not shown: $("$BIN/probe" "$s8" 3)"
ctl next -o TEST-8 && expect "$shown" "$blue"
retry 10 color "255, 0, 0, 255" || fail "blue is not shown: $("$BIN/probe" "$s8" 3)"
ctl prev -o TEST-8 && expect "$shown" "$red"
ctl next && ctl next && expect "$shown" "$green"
ctl next -o TEST-8 && expect "$shown" "$red"
if ctl next -o TEST-9 2>/dev/null; then fail "next on an output without playlist"; fi
ctl playlist move Colors 0 2 && expect "$shown" "$red"
ctl next -o TEST-8 && expect "$shown" "$blue"
refused "the playlist changed" Playlists1 RemoveEntries sauas Colors 1 0 1 "$red"
ctl playlist remove-at Colors 0 && expect "$shown" "$green"
[ "$(ctl playlist history -o TEST-8 --json | js "r['entries'][0]['item']['uid'], len(r['entries']) >= 6")" = "('$green', True)" ] ||
  fail "history: $(ctl playlist history -o TEST-8 --json)"
ctl playlist rule Colors interval 10s
[ "$(ctl playlist rule Colors interval)" = 10s ] || fail "rule: $(ctl playlist rule Colors)"
rotated() { [ "$(field "$shown")" = "$red" ]; }
wait_for 20 rotated || fail "the playlist did not rotate on its own: $(ctl state)"
ctl playlist rule Colors interval 0s
if ctl playlist rule Colors interval 5s 2>/dev/null; then fail "an interval below 10s was accepted"; fi
ctl stream TEST-9 160 90 >/dev/null
ctl playlist rule Colors synced true
ctl playlist start Colors
expect "s['outputs']['TEST-9']['item'] == s['outputs']['TEST-8']['item']" True
first=$(field "$shown")
ctl next -o TEST-9
expect "s['outputs']['TEST-8']['item'] == s['outputs']['TEST-9']['item'] != '$first'" True
ctl lib tag "$red" "$blue" --add warm
ctl playlist new Warm --query "tag:warm" >/dev/null
[ "$(ctl playlist show Warm --json | js "r['total'], r['playlist']['query']")" = "(2, 'tag:warm')" ] ||
  fail "smart playlist: $(ctl playlist show Warm --json)"
ctl playlist start Warm -o TEST-8
expect "$shown in ('$red', '$blue'), s['outputs']['TEST-8']['playlist']['count']" "(True, 2)"
ctl lib tag "$green" --add warm
expect "s['outputs']['TEST-8']['playlist']['count']" 3
before=$(field "$shown")
ctl next -o TEST-8 && expect "$shown != '$before'" True
kept=$(field "$shown")
ctl playlist rule Warm interval 10s
ctl lib tag "$red" "$blue" "$green" --remove warm
expect "s['outputs']['TEST-8']['playlist']['count'], $shown" "(0, '$kept')"
sleep 11
ticks() { awk '{print $14 + $15}' "/proc/$daemon/stat"; }
idle=$(ticks) && sleep 2 && idle=$(($(ticks) - idle))
[ "$idle" -lt 50 ] || fail "the daemon used $idle ticks in 2 s while an empty playlist was due"
ctl lib tag "$red" --add warm
expect "s['outputs']['TEST-8']['playlist']['count'], $shown" "(1, '$red')"
kept=$red
ctl playlist stop -o TEST-8
expect "s['outputs']['TEST-8']['playlist'], $shown" "(None, '$kept')"
ctl playlist rm Warm
[ "$(ctl playlist ls --json | js "[p['name'] for p in r['playlists']]")" = "['Colors']" ] || fail "playlists: $(ctl playlist ls)"
echo "playlist checks passed"

ctl lib apply "$red" -o TEST-8 -f center
ctl filter warm --strength 0.5 -o TEST-8
profile=$(ctl profile save Evening)
ctl lib apply "$green" -o TEST-8 && expect "$shown" "$green"
ctl filter none -o TEST-8
ctl profile use Evening
expect "$shown, s['outputs']['TEST-8']['fit'], s['profile']" "('$red', 'center', '$profile')"
expect "s['outputs']['TEST-8']['filter']['look'], s['outputs']['TEST-8']['filter']['strength']" "('warm', 0.5)"
[ "$(ctl profile save evening)" = "$profile" ] || fail "saving a profile again changed its uid"
[ "$(ctl profile ls --json | js "[(p['name'], p['active'], p['outputs']['TEST-8']['item']) for p in r['profiles']]")" = \
  "[('Evening', True, '$red')]" ] || fail "profiles: $(ctl profile ls --json)"
ctl profile off && expect "s['profile']" None
ctl profile auto Evening outputs TEST-8
[ "$(ctl profile auto Evening outputs)" = TEST-8 ] || fail "auto rule: $(ctl profile auto Evening)"
ctl lib apply "$green" -o TEST-8 && expect "$shown" "$green"
ctl release TEST-9
activated() { [ "$(field "s['profile'], $shown")" = "('$profile', '$red')" ]; }
wait_for 10 activated || fail "the profile was not activated for its outputs: $(ctl state)"
preset=$(ctl preset save "$red" Dim fit=tile background=#102030)
ctl lib apply "$red" -o TEST-8 -p Dim
expect "s['outputs']['TEST-8']['fit'], s['outputs']['TEST-8']['background'], s['outputs']['TEST-8']['preset']" \
  "('tile', '#102030', '$preset')"
[ "$(ctl preset save "$red" dim fit=fit)" = "$preset" ] || fail "saving a preset again changed its uid"
expect "s['outputs']['TEST-8']['fit'], s['outputs']['TEST-8']['background']" "('fit', '#102030')"
ctl preset rm "$red:Dim"
expect "s['outputs']['TEST-8']['preset'], s['outputs']['TEST-8']['fit']" "(None, 'fit')"
ctl preset save - Wide fit=fill >/dev/null
[ "$(ctl preset ls --json | js "[(p['name'], p['item'], p['value']['fit']) for p in r['presets']]")" = "[('Wide', None, 'fill')]" ] ||
  fail "global presets: $(ctl preset ls --json)"
morning=$(ctl profile save Morning)
ctl profile use Morning && expect "s['profile']" "$morning"
[ "$(stat -c %a "$XDG_DATA_HOME/wallpaper-rs/profile-hold.json")" = 600 ] || fail "the manual profile choice is not private"
ctl release TEST-8 && expect "list(s['outputs'])" "[]"
ctl lib unsource "$rot_source"
echo "profile and preset checks passed"
jq_() { python3 -c "import json, sys; r = json.load(sys.stdin); print(eval(sys.argv[1]))" "$1"; }
config=$XDG_CONFIG_HOME/wallpaper-rs/config.toml
daily=$(ctl provider add daily bing refresh=0s count=2)
[ "$(ctl provider ls --json | jq_ "[(p['id'], p['state'], p['hosts'], p['next_sync'], p['source']) for p in r['providers']]")" = \
  "[('daily', 'idle', ['www.bing.com'], None, '$daily')]" ] || fail "providers: $(ctl provider ls --json)"
ctl provider set daily count 3
grep -q '^\[providers.daily\]' "$config" || fail "provider config: $(cat "$config")"
grep -q '^count = 3' "$config" || fail "provider option: $(cat "$config")"
[ "$(ctl lib sources --json | jq_ "[s['provider'] for s in r['sources']]")" = "['daily']" ] || fail "provider source"
refused "belongs to provider" Library1 RemoveSource s "$daily"
if ctl provider set daily count 9 2>"$WORK/provider.out"; then fail "an invalid provider option was accepted"; fi
if ctl provider add daily bing 2>/dev/null; then fail "a provider id was added twice"; fi
if printf 'hunter2\n' | ctl provider add photos webdav url=https://dav.example.org/files user=me 2>"$WORK/keyring.out"; then
  fail "a secret was accepted without a keyring"
fi
grep -q "no Secret Service is available" "$WORK/keyring.out" || fail "keyring message: $(cat "$WORK/keyring.out")"
grep -q photos "$config" && fail "a provider was created although its secret could not be stored"
ctl provider add photos webdav url=https://dav.example.org/files user=me --no-secret >/dev/null
[ "$(ctl provider ls --json | jq_ "[p['state'] for p in r['providers'] if p['id'] == 'photos']")" = "['needs_secret']" ] ||
  fail "provider without secret: $(ctl provider ls --json)"
if ctl provider sync photos 2>"$WORK/sync.out"; then fail "a provider synced without its secret"; fi
grep -q "needs its secret" "$WORK/sync.out" || fail "sync message: $(cat "$WORK/sync.out")"
if printf 'hunter2\n' | ctl provider secret photos 2>"$WORK/secret.out"; then fail "a secret was stored without a keyring"; fi
grep -q "no Secret Service is available" "$WORK/secret.out" || fail "secret message: $(cat "$WORK/secret.out")"
if ctl provider set photos url http://dav.example.org/files 2>"$WORK/plain.out"; then fail "plain http was accepted"; fi
grep -q "plain http" "$WORK/plain.out" || fail "plain http message: $(cat "$WORK/plain.out")"
if ctl provider add leak webdav url=https://me:pw@dav.example.org user=me --no-secret 2>"$WORK/url.out"; then
  fail "credentials in a URL were accepted"
fi
grep -q "not in the URL" "$WORK/url.out" || fail "URL message: $(cat "$WORK/url.out")"
grep -q hunter2 "$config" "$WORK"/*.log && fail "a secret reached the configuration or a log"
ctl provider wallhaven add art --categories general,people --purity sfw --sorting random --top-range 3M --ratios 16x9,landscape \
  --colors 660000,ffffff --resolutions 1920x1080,2560x1440 --pages 2 --max-items 48 --refresh 0s --no-secret >/dev/null
art() { ctl provider ls --json | jq_ "[p['options']['$1'] for p in r['providers'] if p['id'] == 'art'][0]"; }
[ "$(art categories)/$(art purity)/$(art ratios)/$(art colors)/$(art pages)" = "101/100/16x9,landscape/660000,ffffff/2" ] ||
  fail "wallhaven options: $(ctl provider ls --json)"
grep -q '^ratios = "16x9,landscape"' "$config" || fail "wallhaven config: $(cat "$config")"
ctl provider wallhaven set art --similar 94x38z --query "" --sorting date_added --order asc
[ "$(art similar)/$(art order)/$(art sorting)" = "94x38z/asc/date_added" ] || fail "wallhaven update: $(ctl provider ls --json)"
if ctl provider wallhaven set art --collection alice/7 2>"$WORK/wh.out"; then fail "a collection was accepted with a similar search"; fi
grep -q "takes no search terms" "$WORK/wh.out" || fail "collection message: $(cat "$WORK/wh.out")"
[ "$(art collection)" = "" ] || fail "a refused update changed an option"
ctl provider wallhaven set art --similar "" --collection alice/7
[ "$(art collection)" = "alice/7" ] || fail "wallhaven collection: $(ctl provider ls --json)"
for bad in "ratios wide" "colors 123456" "resolutions big" "sorting newest" "pages 11" "collection 12" "tag anime"; do
  read -ra pair <<<"$bad"
  if ctl provider set art "${pair[@]}" 2>/dev/null; then fail "wallhaven accepted $bad"; fi
done
if ctl provider wallhaven set art --sorting newest 2>/dev/null; then fail "the command line accepted a wrong sorting"; fi
if ctl provider wallhaven reseed daily 2>"$WORK/wh.out"; then fail "a provider without a seed was reseeded"; fi
grep -q "not a wallhaven provider" "$WORK/wh.out" || fail "reseed message: $(cat "$WORK/wh.out")"
if ctl provider wallhaven collections art 2>"$WORK/wh.out"; then fail "collections were listed without a user or a key"; fi
grep -q "need a user name or an API key" "$WORK/wh.out" || fail "collections message: $(cat "$WORK/wh.out")"
ctl provider wallhaven set art --purity sketchy,sfw --collection "" --query sea
if ctl provider sync art --wait 2>"$WORK/wh.out"; then fail "sketchy pictures were listed without an API key"; fi
grep -q "need an API key" "$WORK/wh.out" || fail "purity message: $(cat "$WORK/wh.out")"
ctl provider rm art
echo "wallhaven option checks passed"
ctl provider rm photos && ctl provider rm daily --purge
[ "$(ctl provider ls --json | jq_ "len(r['providers'])")" = 0 ] || fail "providers were kept"
[ ! -e "$XDG_CACHE_HOME/wallpaper-rs/providers/daily" ] || fail "the provider cache was kept"
echo "provider checks passed"
