#!/usr/bin/env bash
set -euo pipefail
site=$WORK/site
mkdir -p "$site/src"
cp "$ROOT/tests/fixtures/green-64x32.png" "$site/src/image.png"
cp "$ROOT/tests/fixtures/green-64x32.png" "$site/src/preview.png"
cat >"$site/src/manifest.json" <<'MANIFEST'
{"schema": "wallpaper-rs.wpak/1", "id": "org.example.dawn", "version": "1.0.0", "title": "Dawn",
 "license": "CC0-1.0", "kind": "image", "entry": "image.png", "preview": "preview.png", "tags": ["sky"]}
MANIFEST
(
  cd "$site"
  ctl repo keygen root --no-password >/dev/null
  ctl repo keygen online --no-password >/dev/null
  ctl pack build src -o dawn.wpak >/dev/null
  ctl pack build "$ROOT/scenes/dusk" -o dusk.wpak >/dev/null
  ctl repo init repo --name Example --root-key root.pub --online-key online.pub >/dev/null 2>&1
  ctl repo sign-root repo --key root.key >/dev/null
  ctl repo add repo dawn.wpak >/dev/null
  ctl repo add repo dusk.wpak >/dev/null
  ctl repo publish repo --key online.key >/dev/null
)
anchor=$(ctl repo status "$site/repo" | sed -n 's/^anchor = //p')
port=$(python3 -c "import socket; s = socket.socket(); s.bind(('127.0.0.1', 0)); print(s.getsockname()[1])")
python3 -m http.server --bind 127.0.0.1 --directory "$site/repo" "$port" >"$WORK/http.log" 2>&1 & pids+=($!)
wait_for 10 python3 -c "import urllib.request; urllib.request.urlopen('http://127.0.0.1:$port/entry/latest')" ||
  fail "the repository server did not start"
if ctl remote add "http://127.0.0.1:$port/" 2>"$WORK/anchor.out"; then fail "a repository without anchor was added"; fi
grep -q "fingerprint or repo_id" "$WORK/anchor.out" || fail "anchor message: $(cat "$WORK/anchor.out")"
repo=$(ctl remote add "http://127.0.0.1:$port/$anchor" --wait --json | jq_ "r['output']['repo_id']")
[ "$(ctl remote ls --json | jq_ "[(x['name'], x['hosts'], x['packages'], x['stale']) for x in r['repos']]")" = \
  "[('Example', ['127.0.0.1'], 2, False)]" ] || fail "repositories: $(ctl remote ls --json)"
[ "$(ctl pkg search sky --json | jq_ "[(p['id'], p['version'], p['preview'], p['repo_name']) for p in r['packages']]")" = \
  "[('org.example.dawn', '1.0.0', True, 'Example')]" ] || fail "browse: $(ctl pkg search sky --json)"
preview=$(ctl pkg preview "${repo:0:8}/org.example.dawn")
[ "$(head -c 3 "$preview" | od -An -tx1 | tr -d ' ')" = ffd8ff ] || fail "the preview is not a re-encoded JPEG"
[ "$(stat -c %a "$preview") $(stat -c %a "$(dirname "$preview")")" = "600 700" ] || fail "previews are not private"
[ "$(ctl pkg preview org.example.dawn)" = "$preview" ] || fail "the cached preview was not reused"
ctl remote refresh --wait >/dev/null
item=$(ctl pkg install org.example.dawn --wait --json | jq_ "r['output']['item']")
[ "$(ctl pkg ls --json | jq_ "[(p['id'], p['version'], p['item']) for p in r['packages']]")" = \
  "[('org.example.dawn', '1.0.0', '$item')]" ] || fail "installed: $(ctl pkg ls --json)"
[ "$(ctl lib show "$item" --json | jq_ "r['installed']['version'], r['item']['title'], 'sky' in r['item']['tags']")" = \
  "('1.0.0', 'Dawn', True)" ] || fail "installed item: $(ctl lib show "$item" --json)"
refused "use Repos1.Uninstall" Library1 Remove asb 1 "$item" false
ctl pkg uninstall org.example.dawn
[ "$(ctl pkg ls --json | jq_ "len(r['packages'])")" = 0 ] || fail "the package was kept"
[ "$(ctl lib search kind:package --json | jq_ "r['total']")" = 0 ] || fail "the package item was kept"
dusk=io.github.wallpaper_rs.example.dusk
[ "$(ctl pkg search dusk --json | jq_ "[(p['id'], p['kind']) for p in r['packages']]")" = "[('$dusk', 'scene')]" ] ||
  fail "browse scenes: $(ctl pkg search dusk --json)"
item=$(ctl pkg install "$dusk" --wait --json | jq_ "r['output']['item']")
[ "$(ctl lib search kind:scene --json | jq_ "[i['uid'] for i in r['items']]")" = "['$item']" ] ||
  fail "installed scene: $(ctl lib search kind:scene --json)"
ctl pkg uninstall "$dusk"
python3 -c "import socket, time; s = socket.socket(); s.bind(('127.0.0.1', 0)); s.listen(8)
print(s.getsockname()[1], flush=True); time.sleep(600)" >"$WORK/stall.port" & pids+=($!)
wait_for 5 test -s "$WORK/stall.port" || fail "the stalled server did not start"
stalled=$(bus Repos1 Add s "http://127.0.0.1:$(cat "$WORK/stall.port")/?fingerprint=$(printf '%064d' 0)" | out "d[0]")
running() { [ "$(reply "r['job']['state']" Daemon1 GetJob t "$1")" = running ]; }
wait_for 10 running "$stalled" || fail "the stalled repository job: $(bus Daemon1 GetJob t "$stalled")"
SECONDS=0
ctl remote rm "${repo:0:8}"
[ "$SECONDS" -lt 5 ] || fail "removing a repository waited for the job of another repository"
bus Daemon1 CancelJob t "$stalled" >/dev/null
[ "$(ctl remote ls --json | jq_ "len(r['repos'])")" = 0 ] || fail "the repository was kept"
[ ! -e "$preview" ] || fail "the preview was kept with its repository"
echo "repository checks passed"
looks() {
  read -r b g r _ < <("$BIN/probe" "$1" 3 | sed 's/.*center bgra \[\(.*\)\]/\1/; s/,/ /g')
  near "$r" "$2" && near "$g" "$3" && near "$b" "$4"
}
shown() { wait_for 10 looks "$@" || fail "stream $1 shows $("$BIN/probe" "$1" 3), expected $2 $3 $4"; }
pic=$WORK/edits/meadow.png
mkdir -p "$WORK/edits"
cp "$ROOT/tests/fixtures/green-64x32.png" "$pic"
ctl edit set "$pic" ev=-1
[ "$(stat -c %a "$pic.wprs.json")" = 600 ] || fail "the edit file is not private"
read -r _ _ _ edited < <(ctl stream TEST-8 160 90)
ctl set "$pic" -o TEST-8 --fit stretch
shown "$edited" 0 188 0
expect "s['outputs']['TEST-8']['edited']" True
ctl edit set "$pic" ev=-2
shown "$edited" 0 137 0
ctl edit preview "$pic" ev=0
shown "$edited" 0 255 0
ctl edit preview "$pic" --stop
shown "$edited" 0 137 0
ctl edit fit "$pic" center
expect "s['outputs']['TEST-8']['fit'], s['outputs']['TEST-8']['shown_fit']" "('stretch', 'center')"
ctl edit fit "$pic" none
expect "s['outputs']['TEST-8']['fit'], s['outputs']['TEST-8']['shown_fit']" "('stretch', 'stretch')"
ctl edit overlay "$pic" '#ff0000'
shown "$edited" 255 0 0
ctl edit reset "$pic"
shown "$edited" 0 255 0
expect "s['outputs']['TEST-8']['edited']" False
if [ "$(id -u)" != 0 ]; then
  chmod 555 "$WORK/edits"
  ctl edit set "$pic" ev=-1
  if [ -e "$pic.wprs.json" ] || ! compgen -G "$XDG_DATA_HOME/wallpaper-rs/edits/*.json" >/dev/null; then
    fail "the edit of an image in a read-only folder is not in the data folder"
  fi
  shown "$edited" 0 188 0
  chmod 755 "$WORK/edits"
fi
ctl release TEST-8 && expect "list(s['outputs'])" "[]"
cp "$ROOT/tests/fixtures/bands-640x180.png" "$WORK/bands.png"
"$BIN/wprs-ctl" stream TEST-9 160 90 --wait >"$WORK/span.out" 2>&1 & spanned=$!; pids+=("$spanned")
wait_for 10 grep -q serial "$WORK/span.out" || fail "span claim: $(cat "$WORK/span.out")"
read -r _ _ _ left <"$WORK/span.out"
read -r _ _ _ right < <(ctl stream TEST-10 160 90)
ctl screen TEST-9 --pos 0,0 --size 160x90 --density 4 --identity "Maker Model 9"
ctl screen TEST-10 --pos 160,0 --size 160x90 --density inf
expect "s['outputs']['TEST-10']['screen']['px_per_mm'], s['outputs']['TEST-10']['screen']['x']" "(0.0, 160)"
ctl screen TEST-10 --pos 160,0 --size 160x90 --density 4
ctl set "$WORK/bands.png" --fit fill
shown "$left" 0 255 0 && shown "$right" 0 255 0
ctl fit span
shown "$left" 255 0 0 && shown "$right" 0 0 255
expect "s['outputs']['TEST-9']['screen']['identity']" "Maker Model 9"
ctl edit span "$WORK/bands.png" --zoom 2
shown "$left" 0 255 0 && shown "$right" 0 255 0
ctl edit crop "$WORK/bands.png" 0.7,0,0.3,1 --output TEST-10
shown "$right" 0 0 255
ctl edit reset "$WORK/bands.png"
shown "$left" 255 0 0 && shown "$right" 0 0 255
kill -KILL "$spanned"
wait "$spanned" 2>/dev/null || true
shown "$right" 0 255 0
ctl release TEST-10 && expect "list(s['outputs'])" "[]"
echo "edit and span checks passed"
