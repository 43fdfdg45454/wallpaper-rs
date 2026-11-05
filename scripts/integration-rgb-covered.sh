#!/usr/bin/env bash
set -euo pipefail
declare rgbs sr config
cp -r "$rgbs/scene" "$rgbs/pulse"
ctl scene set "$rgbs/pulse" /manifest/id org.example.pulse
ctl scene set "$rgbs/pulse" /scene/timelines '[{"id": "pulse", "duration": 1, "loop": "repeat"}]'
layers=$(ctl scene show "$rgbs/pulse" /scene/layers | python3 -c 'import json, sys
layers = json.load(sys.stdin)
red = {"id": "red", "type": "solid", "color": "#ff0000", "opacity": {"timeline": "pulse", "keys": [[0, 0], [0.5, 1], [1, 0]]}}
print(json.dumps(layers[:1] + [red] + layers[1:]))')
ctl scene set "$rgbs/pulse" /scene/layers "$layers"
ctl scene build "$rgbs/pulse" -o "$rgbs/pulse.wpak" >/dev/null || fail "the pulsing RGB scene was not built"
job=$(bus Library1 Import asb 1 "$rgbs/pulse.wpak" false | out "d[0]")
wait_for 20 finished "$job" || fail "pulsing RGB scene import: $(bus Daemon1 GetJob t "$job")"
pulse=$(reply "r['job']['output']['imported'][0]['uid']" Daemon1 GetJob t "$job")
"$BIN/probe" "$sr" 120 --hold >/dev/null 2>&1 & watching=$!; pids+=("$watching")
ctl lib apply "$pulse" -o TEST-RGB
ctl input grant org.example.pulse rgb
painted() { wc -l <"$rgbs/painted"; }
drawn() { field "s['outputs']['TEST-RGB']['frames']"; }
grows() { local before; before=$(painted); sleep 1; [ $(($(painted) - before)) -ge "$1" ]; }
held() { [ "$(ctl rgb covered --json | js "r['when_covered'], r['held']")" = "('$1', $2)" ]; }
wait_for 20 grows 10 || fail "the pulsing scene did not keep the lights moving: $(ctl rgb status)"
held pause False || fail "wprs-ctl rgb covered: $(ctl rgb covered --json)"
ctl covered TEST-RGB true && expect "s['outputs']['TEST-RGB']['active']" False
wait_for 10 held pause True || fail "the lights did not pause with the screen covered: $(ctl rgb status)"
ctl rgb status | grep -q "^while covered (rgb.when_covered): pause, paused now" || fail "wprs-ctl rgb status: $(ctl rgb status)"
sleep 0.5
before=$(painted) frames=$(drawn)
sleep 2
[ "$(painted)" = "$before" ] || fail "colors reached the lights while the screen was covered and they pause"
[ "$(drawn)" = "$frames" ] || fail "a covered screen was drawn while its lights paused"
ctl covered TEST-RGB false
wait_for 20 grows 10 || fail "the lights did not resume when the screen showed again"
held pause False || fail "the lights stayed paused on a shown screen"
ctl rgb covered follow | grep -q "^while covered (rgb.when_covered): follow$" || fail "wprs-ctl rgb covered follow: $(ctl rgb covered)"
grep -q '^when_covered = "follow"' "$config" || fail "the choice was not saved: $(cat "$config")"
ctl covered TEST-RGB true && expect "s['outputs']['TEST-RGB']['active']" False
sleep 0.5
before=$(painted) frames=$(drawn)
sleep 2
sent=$(($(painted) - before))
if [ "$sent" -lt 20 ] || [ "$sent" -gt 140 ]; then fail "following lights got $sent colors in 2 s while covered"; fi
[ "$(drawn)" = "$frames" ] || fail "the covered screen was drawn while its lights followed the scene"
held follow False || fail "following lights said they paused"
ctl rgb covered pause >/dev/null
wait_for 10 held pause True || fail "pausing while covered did not hold the lights"
if ctl rgb covered sometimes 2>/dev/null; then fail "wprs-ctl rgb covered took an invalid choice"; fi
if ctl config rgb.when_covered always 2>/dev/null; then fail "an invalid rgb.when_covered was accepted"; fi
[ "$(ctl config rgb.when_covered)" = "rgb.when_covered = \"pause\"" ] || fail "rgb.when_covered: $(ctl config rgb.when_covered)"
ctl covered TEST-RGB false
kill "$watching" 2>/dev/null || true
ctl input revoke org.example.pulse rgb
bus Library1 Remove asb 1 "$pulse" false >/dev/null
echo "rgb lights while covered checks passed"
