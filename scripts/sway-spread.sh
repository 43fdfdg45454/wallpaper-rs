#!/usr/bin/env bash
set -euo pipefail
declare WORK
mkdir -p "$WORK/spread"
cat >"$WORK/spread/scene.json" <<'SCENE'
{"schema": "wallpaper-rs.scene/1", "canvas": {"size": [128, 36], "background": "#000000"},
 "layers": [{"id": "west", "type": "solid", "color": "#ff0000", "size": [64, 36], "transform": {"position": [32, 18]}},
  {"id": "east", "type": "solid", "color": "#0000ff", "size": [64, 36], "transform": {"position": [96, 18]}}]}
SCENE
cat >"$WORK/spread/manifest.json" <<'MANIFEST'
{"schema": "wallpaper-rs.wpak/2", "id": "org.example.spread", "version": "1.0.0", "title": "Spread", "license": "CC0-1.0",
 "kind": "scene", "entry": "scene.json", "content": {"rating": "general"}, "photosensitive": false,
 "origin": {"kind": "original"}, "display": {"span": "all-outputs"}, "requires": {"features": ["spanned_scenes"]}}
MANIFEST
ctl pack build "$WORK/spread" -o "$WORK/spread.wpak" >/dev/null
ctl lib import "$WORK/spread.wpak" --wait >/dev/null
pick='import json, sys; print(next(i["uid"] for i in json.load(sys.stdin)["items"] if i["title"] == "Spread"))'
spread=$(ctl lib search kind:scene --json | python3 -c "$pick")
ctl lib show "$spread" --json | python3 -c "import json, sys; sys.exit(not json.load(sys.stdin)['about']['spread'])" \
  || fail "the package does not say that it spreads"
ctl lib apply "$spread" --span
expect "[s['outputs'][o]['shown_fit'] for o in ('HEADLESS-1', 'HEADLESS-2')]" "['span', 'span']"
check HEADLESS-1 "0.3,0.5,#ff0000" "0.95,0.5,#0000ff"
check HEADLESS-2 "0.05,0.5,#0000ff" "0.95,0.5,#0000ff"
ctl lib apply "$spread" --fit fill
check HEADLESS-2 "0.05,0.5,#ff0000" "0.95,0.5,#0000ff"
ctl lib apply "$spread" --span
check HEADLESS-2 "0.05,0.5,#0000ff"
expect "s['outputs']['HEADLESS-1']['fit']" span
if ctl lib apply "$spread" --span --fit fill 2>/dev/null; then fail "--span was taken with another placement"; fi
ctl scene preview "$WORK/spread" --layout mine --out "$WORK/mine.png" >/dev/null
ctl scene preview "$WORK/spread" --layout dual --out "$WORK/dual.png" >/dev/null
python3 - "$WORK/mine.png" "$WORK/dual.png" <<'PY'
import struct, sys
for path in sys.argv[1:]:
    data = open(path, "rb").read()
    width, height = struct.unpack(">II", data[16:24])
    if width <= height:
        sys.exit(f"{path}: {width}x{height} is not wider than tall")
PY
if ctl scene preview "$WORK/spread" --layout dual 2>/dev/null; then fail "--layout was taken without --out"; fi
if ctl scene preview "$WORK/spread" --layout quad --out "$WORK/q.png" 2>/dev/null; then fail "an unknown layout was drawn"; fi
if ctl scene preview "$WORK/spread" --layout dual --out "$WORK/q.txt" 2>/dev/null; then fail "a text file was written"; fi
ctl scene keyframe "$WORK/spread" /scene/layers/0/color 0 '#ff0000' >/dev/null
ctl scene keyframe "$WORK/spread" /scene/layers/0/color 2 '#00ff00' >/dev/null
for at in poster 0 2; do
  case $at in poster) when=() ;; *) when=(--time "$at") ;; esac
  ctl scene preview "$WORK/spread" --layout dual --out "$WORK/at-$at.png" "${when[@]}" >/dev/null || fail "no layout preview at $at"
done
cmp -s "$WORK/at-poster.png" "$WORK/at-0.png" || fail "--time 0 did not draw the poster time"
if cmp -s "$WORK/at-0.png" "$WORK/at-2.png"; then fail "--time 2 drew the same moment as --time 0"; fi
ctl lib apply "$spread" --fit fill
ctl lib rm "$spread"
echo "spread scene checks passed"
