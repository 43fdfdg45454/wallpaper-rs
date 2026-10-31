#!/usr/bin/env bash
set -euo pipefail
pics=$WORK/pictures
mkdir -p "$pics/videos" "$pics/.hidden" "$WORK/import"
for name in harbour dunes forest; do cp "$ROOT/tests/fixtures/green-64x32.png" "$pics/$name.png"; done
cp "$ROOT/tests/fixtures/magenta-64x32.webm" "$pics/videos/clip.webm"
cp "$ROOT/tests/fixtures/green-64x32.png" "$pics/.hidden/skipped.png"
cp "$ROOT/tests/fixtures/green-64x32.png" "$WORK/import/sunset.png"
dbus-monitor --session "type='signal',interface='io.github.wallpaper_rs.Daemon1'" \
  "type='signal',interface='io.github.wallpaper_rs.Library1'" >"$WORK/library-signals.log" 2>&1 & pids+=($!)
sleep 0.5
[ "$(reply "len(r['options']), len(r['providers']), r['thumb_sizes']" Daemon1 GetSchema)" = "(18, 7, [256, 512, 1024])" ] ||
  fail "schema: $(bus Daemon1 GetSchema)"
read -r source job < <(bus Library1 AddSource sb "$pics" true | out "' '.join(map(str, d))")
wait_for 20 finished "$job" || fail "scan job: $(bus Daemon1 GetJob t "$job")"
wait_for 5 signalled JobProgress "uint64 $job" || fail "no progress for job $job"
wait_for 5 signalled JobFinished "string \"done\"" || fail "no JobFinished for job $job"
refused "overlaps the source" Library1 AddSource sb "$pics/videos" false
refused "contains wallpaper-rs data" Library1 AddSource sb "$WORK" true
[ "$(reply "r['total']" Library1 Search suu "" 0 50)" = 4 ] || fail "search: $(bus Library1 Search suu "" 0 50)"
[ "$(reply "r['total']" Library1 Search suu "kind:video" 0 0)" = 1 ] || fail "video search"
read -r dunes forest harbour < <(reply "' '.join(i['uid'] for i in r['items'])" Library1 Search suu "kind:image sort:title" 0 3)
clip=$(reply "r['items'][0]['uid']" Library1 Search suu "kind:video" 0 1)
[ "$(bus Library1 Resolve as 1 "${harbour: -8}" | out "d[0][0]")" = "$harbour" ] || fail "resolve by suffix"
refused "no item matches \"ZZZZZZZZ\"" Library1 Resolve as 1 ZZZZZZZZ
bus Library1 SetTags asasas 1 "$harbour" 1 sea 0 >/dev/null
bus Library1 SetRating asu 2 "$harbour" "$dunes" 4 >/dev/null
bus Library1 SetFavorite asb 1 "$harbour" true >/dev/null
refused "rating must be 0 to 5" Library1 SetRating asu 1 "$harbour" 6
[ "$(reply "r['tags']" Library1 ListTags)" = "[{'name': 'sea', 'count': 1}]" ] || fail "tags: $(bus Library1 ListTags)"
[ "$(reply "[i['uid'] for i in r['items']]" Library1 Search suu "tag:sea rating:4 fav:" 0 5)" = "['$harbour']" ] ||
  fail "search by tag, rating and favorite"
wait_for 30 idle || fail "analysis did not finish: $(bus Daemon1 ListJobs)"
[ "$(reply "r['item']['width'], r['item']['color'], r['item']['path'].endswith('/harbour.png')" Library1 GetItem s "$harbour")" = \
  "(64, 65280, True)" ] || fail "analysis: $(bus Library1 GetItem s "$harbour")"
[ "$(reply "r['item']['duration_ms'] > 0, r['item']['audio']" Library1 GetItem s "$clip")" = "(True, False)" ] ||
  fail "video analysis: $(bus Library1 GetItem s "$clip")"
thumb=$(bus Library1 Thumbnail su "$harbour" 256 | out "d[0]")
[ "$(head -c 3 "$thumb" | od -An -tx1 | tr -d ' ')" = ffd8ff ] || fail "the thumbnail is not a JPEG"
[ "$(stat -c %a "$thumb") $(stat -c %a "$(dirname "$thumb")")" = "600 700" ] || fail "thumbnails are not private"
[ "$(stat -c %a "$XDG_DATA_HOME/wallpaper-rs/library.db")" = 600 ] || fail "library.db is not private"
[ -s "$(bus Library1 Thumbnail su "$clip" 512 | out "d[0]")" ] || fail "no video thumbnail"
refused "size must be one of" Library1 Thumbnail su "$clip" 300
[ "$(bus Library1 RequestThumbnails asu 2 "$dunes" "$forest" 512 | out "d[0]")" = "{}" ] || fail "thumbnails were cached early"
wait_for 20 signalled ThumbnailReady "$forest" || fail "no ThumbnailReady for $forest"
wait_for 20 signalled ThumbnailReady "$dunes" || fail "no ThumbnailReady for $dunes"
forest_thumb=$(bus Library1 RequestThumbnails asu 1 "$forest" 512 | out "d[0]['$forest']")
serial=$(busctl --user --json=short get-property io.github.wallpaper_rs.Daemon /io/github/wallpaper_rs/Daemon \
  io.github.wallpaper_rs.Daemon1 Serial | out "d")
bus Library1 SetTitle ss "$dunes" "Dunes at noon" >/dev/null
seen=$(for _ in 1 2 3 4 5 6; do
  r=$(bus Daemon1 WaitChanged tu "$serial" 5000); topics=$(echo "$r" | out "d[1]"); echo "$topics"
  case $topics in *library*) break ;; esac
  serial=$(echo "$r" | out "d[0]")
done)
grep -q library <<<"$seen" || fail "WaitChanged missed the library change: $seen"
refused "remove the file or the folder" Library1 Remove asb 1 "$forest" false
rm "$pics/forest.png"
job=$(bus Library1 Rescan s "" | out "d[0]")
wait_for 20 finished "$job" || fail "rescan: $(bus Daemon1 GetJob t "$job")"
[ "$(reply "r['item']['missing']" Library1 GetItem s "$forest")" = True ] || fail "the deleted file is not missing"
bus Library1 Remove asb 1 "$forest" false >/dev/null
[ ! -e "$forest_thumb" ] || fail "the thumbnail of a removed item was kept"
job=$(bus Library1 Import asb 1 "$WORK/import/sunset.png" true | out "d[0]")
wait_for 20 finished "$job" || fail "import: $(bus Daemon1 GetJob t "$job")"
sunset=$(reply "r['job']['output']['imported'][0]['uid']" Daemon1 GetJob t "$job")
copy=$XDG_DATA_HOME/wallpaper-rs/files/sunset.png
[ "$(stat -c %a "$copy")" = 600 ] || fail "the imported copy is not private"
[ "$(reply "r['items'][0]['imported'], r['items'][0]['removable'], r['items'][0]['title']" Library1 Search suu "sunset" 0 1)" = \
  "(True, True, 'sunset')" ] || fail "imported copy: $(bus Library1 Search suu sunset 0 1)"
ctl stream TEST-7 160 90 >/dev/null
bus Library1 Apply ssss "$harbour" TEST-7 fit "" >/dev/null
expect "s['outputs']['TEST-7']['item']" "$harbour"
expect "s['outputs']['TEST-7']['fit']" fit
python3 - "$WORK/import/calm.wpak" "$ROOT/tests/fixtures/green-64x32.png" <<'PACKAGE'
import json, sys, zipfile
manifest = {"schema": "wallpaper-rs.wpak/1", "id": "org.example.calm", "version": "1.0.0", "title": "Calm",
            "license": "CC0-1.0", "kind": "image", "entry": "image.png"}
with zipfile.ZipFile(sys.argv[1], "w") as package:
    package.writestr("manifest.json", json.dumps(manifest))
    package.write(sys.argv[2], "image.png")
PACKAGE
job=$(bus Library1 Import asb 1 "$WORK/import/calm.wpak" false | out "d[0]")
wait_for 20 finished "$job" || fail "package import: $(bus Daemon1 GetJob t "$job")"
calm=$(reply "r['job']['output']['imported'][0]['uid']" Daemon1 GetJob t "$job")
bus Library1 Apply ssss "$calm" TEST-7 "" "" >/dev/null
expect "s['outputs']['TEST-7']['item']" "$calm"
read -r _ _ _ serial7 < <(ctl stream TEST-7 160 90)
retry 10 sh -c "'$BIN/probe' $serial7 3 | grep -q 'center bgra \[0, 255, 0, 255\]'" || fail "the package is not shown"
[ "$(find "$XDG_CACHE_HOME/wallpaper-rs/blobs" -type f | wc -l)" = 1 ] || fail "the package entry was not materialized once"
if ctl set "$WORK/import/calm.wpak" 2>"$WORK/wpak.out"; then fail "SetWallpaper accepted a package"; fi
grep -q "import the package" "$WORK/wpak.out" || fail "package message: $(cat "$WORK/wpak.out")"
mkdir -p "$WORK/storm" && cp "$ROOT/tests/fixtures/green-64x32.png" "$WORK/storm/image.png"
cp "$ROOT/tests/fixtures/green-64x32.png" "$WORK/storm/preview.png"
echo R0lGODlhAQABAIAAAP///wAAACH5BAEAAAAALAAAAAABAAEAAAICRAEAOw== | base64 -d >"$WORK/storm/moving.gif"
cat >"$WORK/storm/manifest.json" <<'STORM'
{"schema": "wallpaper-rs.wpak/2", "id": "org.example.storm", "version": "1.0.0", "default_locale": "en",
 "title": {"en": "Storm", "es": "Tormenta"}, "description": "Lightning over **the sea**.", "license": "CC0-1.0",
 "kind": "image", "entry": "image.png", "previews": {"thumbnail": "preview.png", "animated": "moving.gif"},
 "origin": {"kind": "original"}, "content": {"rating": "mature", "descriptors": ["violence"]}, "photosensitive": true,
 "display": {"background": "#ff0000"}}
STORM
ctl pack check "$WORK/storm" | grep -q '^rating = mature (violence)$' || fail "pack check: $(ctl pack check "$WORK/storm")"
ctl pack build "$WORK/storm" -o "$WORK/import/storm.wpak" >/dev/null
job=$(bus Library1 Import asb 1 "$WORK/import/storm.wpak" false | out "d[0]")
wait_for 20 finished "$job" || fail "rated package import: $(bus Daemon1 GetJob t "$job")"
storm=$(reply "r['job']['output']['imported'][0]['uid']" Daemon1 GetJob t "$job")
[ "$(reply "r['total']" Library1 Search suu "content:mature facet:photosensitive" 0 0)" = 1 ] || fail "rated search"
cmp -s "$(reply "r['about']['animated']" Library1 GetItem s "$storm")" "$WORK/storm/moving.gif" || fail "no animated preview"
ctl config library.hide_mature true
[ "$(reply "r['total']" Library1 Search suu "storm" 0 0), $(reply "r['total']" Library1 Search suu "content:mature" 0 0)" = "0, 1" ] ||
  fail "hidden mature content: $(ctl lib search storm)"
ctl config library.hide_mature false
refused "the user has not allowed it yet" Library1 Apply ssss "$storm" TEST-7 "" ""
if ctl lib apply "$storm" -o TEST-7 </dev/null 2>"$WORK/flash.err"; then fail "a photosensitive package was applied without consent"; fi
grep -q "photosensitive epilepsy.*lib confirm photosensitive" <(tr '\n' ' ' <"$WORK/flash.err") ||
  fail "photosensitivity warning: $(cat "$WORK/flash.err")"
ctl lib show "$storm" | grep -q '^photosensitive = yes, asks before it is shown$' || fail "lib show: $(ctl lib show "$storm")"
ctl lib confirm photosensitive "$storm" && ctl lib apply "$storm" -o TEST-7
expect "s['outputs']['TEST-7']['item'], s['outputs']['TEST-7']['background']" "('$storm', '#000000')"
ctl background '#102030' -o TEST-7 && ctl lib apply "$storm" -o TEST-7
ctl lib apply "$harbour" -o TEST-7
expect "s['outputs']['TEST-7']['item'], s['outputs']['TEST-7']['background']" "('$harbour', '#102030')"
ctl lib apply "$storm" -o TEST-7
mkdir -p "$WORK/flash" && cp "$ROOT/tests/fixtures/green-64x32.png" "$WORK/flash/preview.png"
echo '{"schema": "wallpaper-rs.scene/1", "canvas": {"size": [64, 32]}, "layers": [{"id": "fill", "type": "solid", "color": "#ffffff"}]}' >"$WORK/flash/scene.json"
flash() {
  printf '{"schema": "wallpaper-rs.wpak/2", "id": "%s", "version": "1.0.0", "title": "Flash", "license": "CC0-1.0",
    "kind": "scene", "entry": "scene.json", "previews": {"thumbnail": "preview.png"}, "origin": {"kind": "original"},
    "content": {"rating": "general"}, "photosensitive": true}' "$1" >"$WORK/flash/manifest.json"
  ctl pack build "$WORK/flash" -o "$WORK/import/flash.wpak" >/dev/null
  job=$(bus Library1 Import asb 1 "$WORK/import/flash.wpak" false | out "d[0]")
  wait_for 20 finished "$job" || fail "flashing scene import: $(bus Daemon1 GetJob t "$job")"
}
flash org.example.flash
flashing=$(reply "r['items'][0]['uid']" Library1 Search suu "flash" 0 1)
ctl lib confirm photosensitive "$flashing" && ctl lib apply "$flashing" -o TEST-7
stormy=$(ctl profile save Stormy --no-options)
flash org.example.flash2
ctl profile use "$stormy"
expect "s['outputs']['TEST-7']['item'], 'once you allow it' in (s['outputs']['TEST-7']['error'] or '')" "(None, True)"
ctl stream TEST-20 160 90 >/dev/null && ctl background '#203040' -o TEST-20 && ctl lib confirm photosensitive "$storm"
expect "s['outputs']['TEST-7']['item'], s['outputs']['TEST-20']['background']" "(None, '#203040')"
ctl lib confirm photosensitive "$flashing"
expect "s['outputs']['TEST-7']['item'], s['outputs']['TEST-7']['error'], s['outputs']['TEST-20']['background']" "('$flashing', None, '#203040')"
ctl release TEST-20 && ctl profile off && ctl profile rm "$stormy"
ctl lib apply "$storm" -o TEST-7
expect "s['outputs']['TEST-7']['item'], s['outputs']['TEST-7']['error']" "('$storm', None)"
mkdir -p "$WORK/blink" && cp "$WORK/flash/preview.png" "$WORK/blink/preview.png"
cat >"$WORK/blink/scene.json" <<'BLINK'
{"schema": "wallpaper-rs.scene/1", "canvas": {"size": [64, 32], "background": "#000000"},
 "timelines": [{"id": "blink", "duration": 0.1, "loop": "repeat"}],
 "layers": [{"id": "fill", "type": "solid", "color": "#ffffff",
   "opacity": {"timeline": "blink", "keys": [[0, 0], [0.05, 1], [0.1, 0]]}}]}
BLINK
sed 's/"org.example.flash2"/"org.example.blink"/' "$WORK/flash/manifest.json" >"$WORK/blink/manifest.json"
ctl pack build "$WORK/blink" -o "$WORK/import/blink.wpak" >/dev/null
job=$(bus Library1 Import asb 1 "$WORK/import/blink.wpak" false | out "d[0]")
wait_for 20 finished "$job" || fail "blinking scene import: $(bus Daemon1 GetJob t "$job")"
blink=$(reply "r['job']['output']['imported'][0]['uid']" Daemon1 GetJob t "$job")
ctl lib confirm photosensitive "$blink" && ctl lib apply "$blink" -o TEST-7 && consume "$serial7"
calmed() { [ -n "$(field "s['outputs']['TEST-7']['flashes']")" ]; }
sleep 2
if calmed; then fail "the flash guard acted while it was off: $(ctl state)"; fi
ctl config playback.flash_guard true
wait_for 20 calmed || fail "the flash guard let a scene flash ten times a second: $(ctl state)"
ctl config playback.flash_guard false
expect "s['outputs']['TEST-7']['flashes']" ""
expect "s['outputs']['TEST-7']['left_out']" "[]"
ctl retry-effects || fail "retry-effects failed with a scene shown"
expect "s['outputs']['TEST-7']['left_out']" "[]"
leave && bus Library1 Remove asb 1 "$blink" false >/dev/null
ctl lib apply "$storm" -o TEST-7
echo "flash guard checks passed"
bus Library1 Remove asb 1 "$flashing" false >/dev/null
[ "$(reply "r['item']['package']['confirmed'], r['about']['content'], r['about']['descriptors']" Library1 GetItem s "$storm")" = \
  "(['photosensitive'], 'mature', ['violence'])" ] || fail "rated details: $(ctl lib show "$storm" --json)"
sed -i 's/"1.0.0"/"1.1.0"/' "$WORK/storm/manifest.json"
ctl pack build "$WORK/storm" -o "$WORK/import/storm-1.1.wpak" >/dev/null
job=$(bus Library1 Import asb 1 "$WORK/import/storm-1.1.wpak" false | out "d[0]")
wait_for 20 finished "$job" || fail "newer version import: $(bus Daemon1 GetJob t "$job")"
newer=$(reply "r['job']['output']['imported'][0]['uid']" Daemon1 GetJob t "$job")
wait_for 30 idle || fail "analysis did not finish: $(bus Daemon1 ListJobs)"
sleep 0.5
mark=$(wc -l <"$WORK/library-signals.log")
ctl lib confirm mature "$storm"
told() { tail -n "+$mark" "$WORK/library-signals.log" | grep -A5 "member=Changed" | grep -q "$newer"; }
wait_for 5 told || fail "Confirm did not announce the other version: $(tail -n "+$mark" "$WORK/library-signals.log")"
[ "$(reply "r['item']['package']['confirmed']" Library1 GetItem s "$newer")" = "['mature', 'photosensitive']" ] ||
  fail "the other version does not keep the decisions: $(ctl lib show "$newer" --json)"
ctl release TEST-7 && expect "list(s['outputs'])" "[]"
bus Library1 Remove asb 3 "$newer" "$storm" "$calm" false >/dev/null
[ "$(find "$XDG_CACHE_HOME/wallpaper-rs/blobs" -type f | wc -l)" = 0 ] || fail "the package entry was kept"
bus Library1 Remove asb 1 "$sunset" true >/dev/null
[ ! -e "$copy" ] || fail "the imported copy was not deleted"
bus Library1 RemoveSource s "$source" >/dev/null
[ "$(reply "len(r['sources']), " Library1 ListSources)" = "(0,)" ] || fail "sources: $(bus Library1 ListSources)"
[ "$(reply "r['total']" Library1 Search suu "" 0 0)" = 0 ] || fail "items were kept with their source"
[ ! -e "$thumb" ] || fail "thumbnails were kept with their source"
echo "library checks passed"
