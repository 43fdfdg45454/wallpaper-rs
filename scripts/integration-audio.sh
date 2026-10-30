#!/usr/bin/env bash
set -euo pipefail
declare scene sea
python3 - "$scene" "$ROOT/tests/fixtures/green-64x32.png" <<'PACKAGE'
import json, sys, zipfile
def package(name, scene, files):
    manifest = {"schema": "wallpaper-rs.wpak/1", "id": f"org.example.{name}", "version": "1.0.0", "title": name,
                "license": "CC0-1.0", "kind": "scene", "entry": "scene.json", "preview": "preview.png"}
    scene = {"schema": "wallpaper-rs.scene/1", "canvas": {"size": [64, 32], "fit": "stretch"}, **scene}
    with zipfile.ZipFile(f"{sys.argv[1]}/{name}.wpak", "w") as out:
        out.writestr("manifest.json", json.dumps(manifest))
        out.writestr("scene.json", json.dumps(scene))
        out.write(sys.argv[2], "preview.png")
        for path, text in files.items():
            out.writestr(path, text)
package("sea", {"layers": [{"id": "bars", "type": "spectrum", "size": [64, 32], "bands": 16}]}, {})
package("twin", {"layers": [{"id": "bars", "type": "spectrum", "size": [64, 32], "bands": 32}]}, {})
tint = "import { scene } from 'wprs';\nexport function update() { scene.layer('tint').color = [1, 0, 0, 1]; }\n"
package("spin", {"layers": [{"id": "tint", "type": "solid", "color": "#0000ff"}],
                 "scripts": [{"path": "scripts/tint.js"}]}, {"scripts/tint.js": tint})
PACKAGE
imported() {
  local job
  job=$(bus Library1 Import asb 1 "$1" false | out "d[0]")
  wait_for 20 finished "$job" || fail "import of $1: $(bus Daemon1 GetJob t "$job")"
  reply "r['job']['output']['imported'][0]['uid']" Daemon1 GetJob t "$job"
}
python3 - "$scene/twin.wpak" <<'TWIN'
import json, sys, zipfile
with zipfile.ZipFile(sys.argv[1]) as old:
    files = {name: old.read(name) for name in old.namelist()}
manifest = json.loads(files["manifest.json"])
manifest["id"] = "org.example.sea"
files["manifest.json"] = json.dumps(manifest)
with zipfile.ZipFile(sys.argv[1], "w") as out:
    for name, data in files.items():
        out.writestr(name, data)
TWIN
sea_uid=$(imported "$scene/sea.wpak") && spin_uid=$(imported "$scene/spin.wpak") && twin_uid=$(imported "$scene/twin.wpak")
analysing() {
  local dump
  dump=$(pw-dump 2>/dev/null) || return 1
  grep -q '"media.name": "wallpaper-rs audio analysis"' <<<"$dump"
}
silent() { ! analysing; }
reads() { [ "$(inputs "$1")" = "$2" ]; }
read -r _ _ _ s12 < <(ctl stream TEST-12 160 90)
watch12() { "$BIN/probe" "$s12" 120 --hold >/dev/null 2>&1 & watcher=$!; pids+=("$watcher"); }
watch12
ctl lib apply "$sea_uid" -o TEST-12
wait_for 10 reads "r['shown']['TEST-12']['audio'], r['shown']['TEST-12']['scene']" "(True, '$sea')" ||
  fail "the audio scene was not declared: $(ctl input status --json)"
sleep 1
[ "$(inputs "r['capturing']")" = False ] || fail "audio was captured without permission"
if analysing; then fail "an audio stream exists without permission"; fi
denied "nothing can ask the user\\|is asked in a notification" Inputs1.Grant string:"$sea" string:audio
[ "$(inputs "r['grants']")" = "{}" ] || fail "another program granted an input: $(ctl input status --json)"
ctl input grant "$sea" audio && ctl input grant "$sea" pointer && ctl input grant "$sea" audio
[ "$(inputs "r['grants'], r['shown']['TEST-12']['granted']")" = "({'$sea': ['audio', 'pointer']}, ['audio', 'pointer'])" ] ||
  fail "grants: $(ctl input status --json)"
dbus-send --session --print-reply --dest=io.github.wallpaper_rs.Daemon /io/github/wallpaper_rs/Daemon \
  io.github.wallpaper_rs.Inputs1.Grant string:"$sea" string:audio >/dev/null ||
  fail "granting an allowed input again was refused or asked"
[ "$(stat -c %a "$XDG_DATA_HOME/wallpaper-rs/inputs.json")" = 600 ] || fail "inputs.json is not private"
grep -q '"sha256:' "$XDG_DATA_HOME/wallpaper-rs/inputs.json" || fail "a grant is not tied to the package"
wait_for 10 reads "r['capturing']" True || fail "no audio capture with the permission: $(ctl input status --json)"
wait_for 10 analysing || fail "no PipeWire stream for the audio analysis"
ctl config pause.when_covered false && ctl covered TEST-12 true
wait_for 10 reads "r['capturing']" False || fail "the sound of a covered screen was captured"
ctl covered TEST-12 false && ctl config pause.when_covered true
wait_for 10 reads "r['capturing']" True || fail "the capture did not come back when the screen was shown"
kill "$watcher"
wait_for 10 reads "r['capturing']" False || fail "the sound of a screen nobody watches was captured"
watch12
wait_for 10 reads "r['capturing']" True || fail "the capture did not come back when the screen was watched"
ctl config playback.scene_capture false
wait_for 10 reads "r['capturing']" False || fail "the capture went on after it was turned off for every scene"
wait_for 10 silent || fail "the analysis stream outlived the switch"
ctl config playback.scene_capture true
wait_for 10 reads "r['capturing']" True || fail "the capture did not come back when it was allowed again"
ctl lib apply "$twin_uid" -o TEST-12
wait_for 10 reads "r['shown']['TEST-12']['granted'], r['capturing']" "([], False)" ||
  fail "another package with the same id used the grant: $(ctl input status --json)"
ctl lib apply "$sea_uid" -o TEST-12
wait_for 10 reads "r['capturing']" True || fail "the grant was lost: $(ctl input status --json)"
ctl stream TEST-13 160 90 >/dev/null
ctl lib apply "$twin_uid" -o TEST-13 2>"$WORK/twin.err"
wait_for 10 reads "r['shown']['TEST-13']['scene'], r['shown']['TEST-13']['granted']" "('$sea', [])" ||
  fail "the twin on a second screen: $(ctl input status --json)"
grep -q "wprs-ctl input grant $sea audio -o TEST-13" "$WORK/twin.err" || fail "twin hint: $(cat "$WORK/twin.err")"
if ctl input grant "$sea" audio 2>"$WORK/grant.err"; then fail "a bare id chose one of two packages"; fi
grep -q "choose one with --output (TEST-12, TEST-13)" "$WORK/grant.err" || fail "two packages: $(cat "$WORK/grant.err")"
refused "more than one package with the id $sea" Inputs1 Grant ss "$sea" audio
refused "is not shown" Inputs1 GrantPackage sss "$sea" sha256:00 audio
ctl input grant "$sea" audio -o TEST-12
[ "$(inputs "r['shown']['TEST-12']['granted'], r['shown']['TEST-13']['granted']")" = "(['audio', 'pointer'], [])" ] ||
  fail "granting one package reached its twin: $(ctl input status --json)"
ctl input status | grep -q "^TEST-13 shows $sea using .* (allowed: none)$" || fail "status: $(ctl input status)"
ctl input grant "$sea" audio -o TEST-13
[ "$(inputs "r['shown']['TEST-13']['granted']")" = "['audio']" ] || fail "grant -o: $(ctl input status --json)"
ctl release TEST-13
ctl input revoke "$sea" pointer && ctl input revoke "$sea" pointer
[ "$(inputs "r['grants']")" = "{'$sea': ['audio']}" ] || fail "revoke: $(ctl input status --json)"
ctl lib apply "$spin_uid" -o TEST-12
wait_for 10 reads "r['capturing'], list(r['shown'])" "(False, [])" || fail "the capture outlived its scene"
wait_for 10 silent || fail "the audio stream was kept"
shown "$s12" 255 0 0
expect "s['outputs']['TEST-12']['kind'], s['outputs']['TEST-12']['error']" "('scene', None)"
consume "$s12" && sleep 2
idle=$(ticks) && sleep 2 && idle=$(($(ticks) - idle)) && leave
[ "$idle" -lt 8 ] || fail "the daemon used $idle ticks in 2 s while a watched scripted scene stayed the same"
ctl release TEST-12 && expect "list(s['outputs'])" "[]"
ctl input revoke "$sea" audio
[ "$(inputs "r['grants'], r['capturing'], r['tracking']")" = "({}, False, False)" ] || fail "inputs after revoking"
grep -q "$sea" "$XDG_DATA_HOME/wallpaper-rs/inputs.json" && fail "a revoked grant was kept"
bus Library1 Remove asb 3 "$sea_uid" "$spin_uid" "$twin_uid" false >/dev/null
echo "audio and script scene checks passed"
