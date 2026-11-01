#!/usr/bin/env bash
set -euo pipefail
content=$WORK/content
mkdir -p "$content"
python3 - "$content" "$ROOT/tests/fixtures/green-64x32.png" <<'PACKAGE'
import json, sys, zipfile
common = {"schema": "wallpaper-rs.wpak/2", "license": "CC0-1.0", "photosensitive": False,
          "content": {"rating": "general"}, "origin": {"kind": "original"}, "previews": {"thumbnail": "preview.png"}}
base = dict(common, id="org.example.base", version="1.2.0", title="Base", kind="scene", entry="scene.json",
            properties={"tint": {"type": "color", "label": "Tint", "default": "#0000ff"}},
            display={"accent": {"prop": "tint"}}, playback={"max_fps": 12},
            requires={"features": ["accent", "playback2"]})
scene = {"schema": "wallpaper-rs.scene/1", "canvas": {"size": [64, 32], "fit": "stretch"},
         "layers": [{"id": "tint", "type": "solid", "color": {"prop": "tint"}, "size": [64, 32]}]}
preset = dict(common, id="org.example.base-green", version="1.0.0", title="Base in green", kind="preset",
              entry="preset.json", requires={"features": ["packages", "preset"],
              "packages": [{"id": "org.example.base", "version": "^1.2"}]})
values = {"base": {"id": "org.example.base", "version": "^1.2"}, "values": {"tint": "#00ff00", "gone": 3}}
picture = dict(common, id="org.example.marked", version="1.0.0", title="Marked", kind="image", entry="image.png",
               display={"accent": "#ff0000"}, requires={"features": ["accent", "display2"]})
for name, manifest, files in [("base", base, {"scene.json": scene}), ("green", preset, {"preset.json": values}),
                              ("picture", picture, {})]:
    with zipfile.ZipFile(f"{sys.argv[1]}/{name}.wpak", "w") as package:
        package.writestr("manifest.json", json.dumps(manifest))
        [package.writestr(k, json.dumps(v)) for k, v in files.items()]
        package.write(sys.argv[2], "preview.png")
        if manifest["kind"] == "image":
            package.write(sys.argv[2], "image.png")
PACKAGE
added() {
  local job; job=$(bus Library1 Import asb 1 "$1" false | out "d[0]")
  wait_for 20 finished "$job" || fail "import of $1: $(bus Daemon1 GetJob t "$job")"
  reply "r['job']['output']['imported'][0]['uid']" Daemon1 GetJob t "$job"
}
read -r _ _ _ s20 < <(ctl stream TEST-20 160 90)
green=$(added "$content/green.wpak")
if ctl lib apply "$green" -o TEST-20 2>"$WORK/preset.err"; then fail "a preset was applied without its base"; fi
grep -q "org.example.base" "$WORK/preset.err" || fail "missing base: $(cat "$WORK/preset.err")"
base=$(added "$content/base.wpak")
[ "$(reply "[v['uid'] for v in r['variants']]" Library1 GetItem s "$base")" = "['$green']" ] ||
  fail "variants: $(bus Library1 GetItem s "$base")"
[ "$(reply "r['base']['uid'], r['item']['package']['base']" Library1 GetItem s "$green")" = "('$base', 'org.example.base')" ] ||
  fail "base of the preset: $(bus Library1 GetItem s "$green")"
ctl lib apply "$base" -o TEST-20 && shown "$s20" 0 0 255
expect "s['outputs']['TEST-20']['accent']" "#0000ff"
ctl lib apply "$green" -o TEST-20
expect "s['outputs']['TEST-20']['item'], s['outputs']['TEST-20']['accent']" "('$base', '#00ff00')"
shown "$s20" 0 255 0
ctl filter none --exposure -1 -o TEST-20 && shown "$s20" 0 188 0
expect "s['outputs']['TEST-20']['filter']['exposure']" "-1.0"
ctl filter none -o TEST-20 && shown "$s20" 0 255 0
[ "$(ctl prop get tint -o TEST-20)" = '"#00ff00"' ] || fail "preset values: $(ctl prop ls -o TEST-20)"
grep -q "gone is not a setting of org.example.base" "$WORK"/*.log || fail "an unknown preset value was not reported"
fps=$(field "s['outputs']['TEST-20']['fps']")
{ [ "$fps" -ge 1 ] && [ "$fps" -le 12 ]; } || fail "the author's frame rate cap was not kept: $fps"
marked=$(added "$content/picture.wpak")
ctl lib apply "$marked" -o TEST-20 && shown "$s20" 0 255 0
expect "s['outputs']['TEST-20']['accent']" "#ff0000"
ctl release TEST-20 && expect "list(s['outputs'])" "[]"
kept=$XDG_DATA_HOME/wallpaper-rs/storage/org.example.base.json
mkdir -p "${kept%/*}" && echo '{"package":{"visits":3},"outputs":{}}' >"$kept"
bus Library1 Remove asb 3 "$green" "$base" "$marked" false >/dev/null
[ ! -e "$kept" ] || fail "what the scene scripts stored outlived the package"
echo "content checks passed"
