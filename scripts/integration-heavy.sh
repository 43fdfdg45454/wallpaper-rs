#!/usr/bin/env bash
set -euo pipefail
declare level glow
python3 - "$WORK/scene/heavy.wpak" <<'PACKAGE'
import json, sys, zipfile
manifest = {"schema": "wallpaper-rs.wpak/1", "id": "org.example.heavy", "version": "1.0.0", "title": "Heavy",
            "license": "CC0-1.0", "kind": "scene", "entry": "scene.json"}
blur = lambda i: {"id": f"b{i}", "type": "adjustment", "effects": [{"effect": "blur", "params": {"radius": 30}}]}
fade = {"id": "fade", "type": "solid", "color": "#ffffff", "opacity": {"timeline": "fade", "keys": [[0, 0], [1, 1]]}}
scene = {"schema": "wallpaper-rs.scene/1", "canvas": {"size": [960, 540], "background": "#203040"},
         "clock": {"poster_time": 0.5}, "timelines": [{"id": "fade", "duration": 1.0, "loop": "repeat"}],
         "layers": [fade] + [blur(i) for i in range(250)]}
with zipfile.ZipFile(sys.argv[1], "w") as package:
    package.writestr("manifest.json", json.dumps(manifest))
    package.writestr("scene.json", json.dumps(scene))
PACKAGE
ctl lib import "$WORK/scene/heavy.wpak" --wait >/dev/null
heavy=$(ctl lib search "" --json | python3 -c "import json, sys
print([i['uid'] for i in json.load(sys.stdin)['items'] if (i['package'] or {}).get('id') == 'org.example.heavy'][0])")
ctl lib apply "$heavy" -o TEST-Q
costs="s['outputs']['TEST-Q']['quality_auto'], s['outputs']['TEST-Q']['quality_cost'] > s['outputs']['TEST-Q']['quality_budget']"
frames() { field "s['outputs']['TEST-Q']['frames']"; }
moves() { local before; before=$(frames); sleep 1; [ "$(frames)" -gt "$before" ]; }
expect "s['outputs']['TEST-Q']['kind'], $costs" "('scene', 'still', False)"
moves && fail "automatic quality played a scene too heavy for this device"
ctl quality | grep "^TEST-Q: this scene is too heavy for this device" >/dev/null || fail "wprs-ctl quality: $(ctl quality)"
ctl quality q0 | grep "^TEST-Q: this scene costs .* frames may drop and battery use rise; automatic quality would keep it still" >/dev/null ||
  fail "wprs-ctl quality q0 did not warn: $(ctl quality)"
expect "$level, $costs" "('q0', 'user', 'stop', True)"
wait_for 10 moves || fail "a fixed level left the heavy scene still"
ctl quality min-q1 -o TEST-Q | grep "^TEST-Q: this scene costs" >/dev/null || fail "wprs-ctl quality min-q1: $(ctl quality)"
expect "$level, $costs" "('q1', 'user', 'stop', True)"
wait_for 10 moves || fail "a floor left the heavy scene still"
ctl quality none -o TEST-Q >/dev/null && ctl quality auto >/dev/null
expect "$costs" "('still', False)"
sleep 1
moves && fail "going back to automatic quality kept the heavy scene moving"
ctl lib apply "$glow" -o TEST-Q
expect "$costs" "('', False)"
echo "heavy scenes stop only under automatic quality"
