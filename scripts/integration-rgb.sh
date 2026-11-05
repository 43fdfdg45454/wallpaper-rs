#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "$0")"/integration-openrgb.sh
[ "$(ctl rgb status --json | js "r['enabled'], r['connected'], r['local']")" = "(False, False, True)" ] ||
  fail "RGB lighting is not off by default: $(ctl rgb status)"
if ctl rgb devices 2>/dev/null; then fail "RGB lighting connected while off"; fi
if ctl config rgb.server "no port" 2>/dev/null; then fail "a server without a port was accepted"; fi
ctl config rgb.server "lights.example.org:6742"
[ "$(ctl rgb status --json | js "r['local']")" = False ] || fail "a remote server counted as this computer"
ctl config rgb.server "127.0.0.1:$openrgb"
ctl config rgb.enabled true
ctl rgb devices | grep -q "Example keyboard" || fail "no devices: $(ctl rgb status)"
[ "$(ctl rgb devices --json | js "[d['leds'] for d in r]")" = "[4, 1]" ] || fail "devices: $(ctl rgb devices --json)"
ctl rgb devices | grep -q "own mode Rainbow" || fail "the own mode was not shown: $(ctl rgb devices)"
ctl rgb test 1
if ctl rgb test 7 2>/dev/null; then fail "a device that does not exist was tested"; fi
wait_for 10 grep -q '"device": 1' "$rgbs/painted" || fail "the test pattern did not reach the mouse"
ctl scene new "$rgbs/scene" --id org.example.lit >/dev/null
ctl scene set "$rgbs/scene" /scene/canvas/size '[64, 32]'
ctl scene set "$rgbs/scene" /scene/canvas/fit stretch
ctl scene import "$rgbs/scene" "$ROOT/tests/fixtures/green-64x32.png" --id meadow >/dev/null
ctl scene add-layer "$rgbs/scene" image --source meadow >/dev/null
ctl scene add-layer "$rgbs/scene" adjustment --id glow >/dev/null
if ctl scene rgb "$rgbs/scene" image --on true 2>/dev/null; then fail "an image layer was made to send colors"; fi
if ctl scene rgb "$rgbs/scene" glow --size 0x4 2>/dev/null; then fail "an empty region was accepted"; fi
ctl scene rgb "$rgbs/scene" glow --on true --size 32x8
[ "$(ctl scene show "$rgbs/scene" /manifest/permissions/rgb)" = true ] || fail "the permission was not set"
ctl scene build "$rgbs/scene" -o "$rgbs/lit.wpak" >/dev/null || fail "the RGB scene was not built"
read -r _ _ _ sr < <(ctl stream TEST-RGB 160 90)
job=$(bus Library1 Import asb 1 "$rgbs/lit.wpak" false | out "d[0]")
wait_for 20 finished "$job" || fail "RGB scene import: $(bus Daemon1 GetJob t "$job")"
rgbuid=$(reply "r['job']['output']['imported'][0]['uid']" Daemon1 GetJob t "$job")
ctl lib apply "$rgbuid" -o TEST-RGB
wait_for 10 sh -c "'$BIN/wprs-ctl' input status | grep -q 'TEST-RGB shows org.example.lit using rgb'" || fail "the rgb input was not declared: $(ctl input status)"
shown "$sr" 0 255 0
if ctl rgb strip -o TEST-RGB 2>/dev/null; then fail "colors were taken before the grant"; fi
ctl input grant org.example.lit rgb
stripped() { "$BIN/probe" "$sr" 3 >/dev/null 2>&1; ctl rgb strip -o TEST-RGB >/dev/null 2>&1; }
wait_for 20 stripped || fail "no strip after the grant: $(ctl rgb status)"
[ "$(ctl rgb strip -o TEST-RGB --json | js "r['width'], r['height'], set(r['colors'])")" = "(32, 8, {'#00ff00'})" ] ||
  fail "strip: $(ctl rgb strip -o TEST-RGB --json)"
green() { tail -n 2 "$rgbs/painted" | grep -q '"device": 0, "colors": \[\[0, 255, 0\], \[0, 255, 0\], \[0, 255, 0\], \[0, 255, 0\]\]'; }
wait_for 10 green || fail "the keyboard did not turn green: $(tail -n 3 "$rgbs/painted")"
[ "$(ctl rgb status --json | js "r['source'], r['frames'] > 0")" = "('TEST-RGB', True)" ] || fail "status: $(ctl rgb status)"
ctl input revoke org.example.lit rgb
sleep 0.5
before=$(wc -l <"$rgbs/painted")
ctl rgb test 1
tested() { [ "$(ctl rgb status --json | js "r['testing']")" = False ]; }
wait_for 10 tested || fail "the test pattern did not end"
sleep 0.5
if tail -n "+$((before + 1))" "$rgbs/painted" | grep -q '"device": 0'; then
  fail "the colors of a revoked package came back after a test"
fi
source "$(dirname "$0")"/integration-rgb-covered.sh
ctl config rgb.enabled false
[ "$(ctl rgb status --json | js "r['enabled'], r['connected']")" = "(False, False)" ] || fail "RGB lighting stayed on"
wait_for 10 grep -q '"mode": 0, "index": 1' "$rgbs/painted" || fail "the keyboard did not get its own mode back"
ctl release TEST-RGB && expect "list(s['outputs'])" "[]"
bus Library1 Remove asb 1 "$rgbuid" false >/dev/null
echo "rgb checks passed"
