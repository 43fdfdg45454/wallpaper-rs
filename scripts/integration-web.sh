#!/usr/bin/env bash
set -euo pipefail
pages=$WORK/pages
mkdir -p "$pages"
from() { python3 -c "import json, sys; r = json.load(sys.stdin); print(eval(sys.argv[1]))" "$1"; }
uid() { ctl lib search "" --json | from "[i['uid'] for i in r['items'] if (i['package'] or {}).get('id') == '$1'][0]"; }
pixel() { "$BIN/probe" "$1" 3 2>/dev/null | grep -q "center bgra \[$2, 255\]"; }
seen() { wait_for 20 pixel "$@" || fail "stream $1 shows $("$BIN/probe" "$1" 3 2>&1 | tail -n 1), expected bgra $2"; }
helpers() { pgrep -u "$(id -u)" -f "$BIN/wprs-web" | grep -c . || true; }
stopped() { [ "$(helpers)" = 0 ]; }
ctl pack build "$ROOT/tests/fixtures/tint" -o "$pages/tint.wpak" >/dev/null
ctl pack build "$ROOT/scenes/lights" -o "$pages/lights.wpak" >/dev/null
ctl lib import "$pages/tint.wpak" "$pages/lights.wpak" --wait >/dev/null
tint=$(uid org.example.tint)
lights=$(uid io.github.wallpaper_rs.example.lights)
[ "$(ctl lib show "$lights" --json | from "r['item']['package']['exposes']")" = "['zone', 'language', 'gpu', 'storage']" ] ||
  fail "exposed by the example page: $(ctl lib show "$lights" --json)"
ctl lib show "$lights" | grep -q "^privacy = This web wallpaper uses your time zone" || fail "no privacy notice in lib show"
read -r _ _ _ web < <(ctl stream TEST-W 320 180)
if ! "$BIN/wprs-web" --check >/dev/null 2>&1; then
  [ "${WPRS_REQUIRE_WEB:-0}" != 1 ] || fail "WPRS_REQUIRE_WEB=1 but web wallpapers cannot run here"
  if ctl lib apply "$tint" -o TEST-W 2>"$pages/refused"; then fail "a web wallpaper was applied without its helper"; fi
  grep -q "Web wallpapers cannot be shown here" "$pages/refused" || fail "unclear refusal: $(cat "$pages/refused")"
  ctl release TEST-W
  echo "web wallpapers refused with a reason where the web engine is missing"
else
  ctl lib apply "$tint" -o TEST-W 2>"$pages/notice"
  grep -q "This web wallpaper uses your time zone, your language, the model of your graphics card and data it keeps" "$pages/notice" ||
    fail "no privacy notice on apply: $(cat "$pages/notice")"
  expect "s['outputs']['TEST-W']['kind']" web
  seen "$web" "255, 0, 0"
  [ "$(helpers)" = 1 ] || fail "$(helpers) web helpers run for one page"
  ctl prop set tint '#00ff00' -o TEST-W
  seen "$web" "0, 255, 0"
  drawn=$(field "s['outputs']['TEST-W']['decode'], s['outputs']['TEST-W']['sandbox']")
  echo "web page drawn (decode, sandbox): $drawn"
  [[ $drawn != *"''"* ]] || fail "no frame or sandbox state: $drawn"
  ctl lib confirm hide_zone "$tint"
  [ "$(ctl lib show "$tint" --json | from "r['item']['package']['exposes']")" = "['language', 'gpu', 'storage']" ] ||
    fail "the time zone is still exposed"
  seen "$web" "0, 255, 0"
  [ -d "$XDG_DATA_HOME/wallpaper-rs/web/org.example.tint" ] || fail "the page kept no storage"
  touch "$XDG_DATA_HOME/wallpaper-rs/web/org.example.tint/kept"
  ctl lib clear-data "$tint"
  [ ! -e "$XDG_DATA_HOME/wallpaper-rs/web/org.example.tint/kept" ] || fail "the data of the page stayed"
  seen "$web" "0, 255, 0"
  ctl lib apply "$lights" -o TEST-W 2>/dev/null
  expect "s['outputs']['TEST-W']['kind'], s['outputs']['TEST-W']['error']" "('web', None)"
  ctl release TEST-W
  wait_for 10 stopped || fail "the web helper outlived its page"
  echo "web wallpaper checks passed"
fi
