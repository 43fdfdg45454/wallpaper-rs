#!/usr/bin/env bash
set -euo pipefail
rgbs=$WORK/rgb
mkdir -p "$rgbs"
python3 -I - "$rgbs" <<'SERVER' &
import json, socket, struct, sys
out = sys.argv[1]
def text(s):
    b = s.encode() + b"\0"
    return struct.pack("<H", len(b)) + b
def blob(name, kind, zones):
    body = struct.pack("<I", kind) + text(name) + b"".join(text(t) for t in ["Example", "A test device", "1.0", "", "local"])
    body += struct.pack("<HI", 1, 0) + text("Direct") + struct.pack("<12I", *[0] * 12) + struct.pack("<H", 0)
    body += struct.pack("<H", len(zones))
    for i, (leds, matrix) in enumerate(zones):
        body += text(f"zone {i}") + struct.pack("<4I", 1, 0, leds, leds)
        if matrix:
            w, h, cells = matrix
            body += struct.pack("<HII", 8 + 4 * len(cells), h, w) + struct.pack(f"<{len(cells)}I", *cells)
        else:
            body += struct.pack("<H", 0)
        body += struct.pack("<H", 1) + text("segment") + struct.pack("<3I", 0, 0, leds)
    total = sum(z[0] for z in zones)
    body += struct.pack("<H", total) + b"".join(text(f"led {i}") + struct.pack("<I", 0) for i in range(total))
    body += struct.pack("<H", total) + b"\0" * 4 * total
    return struct.pack("<I", len(body) + 4) + body
devices = [blob("Example keyboard", 5, [(4, (2, 2, [0, 1, 2, 3]))]), blob("Example mouse", 6, [(1, None)])]
listener = socket.socket()
listener.bind(("127.0.0.1", 0))
listener.listen()
open(f"{out}/port", "w").write(str(listener.getsockname()[1]))
def packet(device, kind, data):
    return b"ORGB" + struct.pack("<III", device, kind, len(data)) + data
while True:
    conn, _ = listener.accept()
    with conn:
        while True:
            head = conn.recv(16, socket.MSG_WAITALL)
            if len(head) < 16:
                break
            device, kind, size = struct.unpack("<III", head[4:])
            data = conn.recv(size, socket.MSG_WAITALL) if size else b""
            if kind == 40:
                conn.sendall(packet(0, 40, struct.pack("<I", 4)))
            elif kind == 0:
                conn.sendall(packet(0, 0, struct.pack("<I", len(devices))))
            elif kind == 1:
                conn.sendall(packet(device, 1, devices[device]))
            elif kind == 1050:
                n = struct.unpack("<H", data[4:6])[0]
                colors = [list(data[6 + 4 * i:9 + 4 * i]) for i in range(n)]
                with open(f"{out}/painted", "a") as f:
                    f.write(json.dumps({"device": device, "colors": colors}) + "\n")
SERVER
pids+=("$!")
wait_for 10 test -s "$rgbs/port" || fail "the OpenRGB test server did not start"
openrgb=$(cat "$rgbs/port")
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
ctl config rgb.enabled false
[ "$(ctl rgb status --json | js "r['enabled'], r['connected']")" = "(False, False)" ] || fail "RGB lighting stayed on"
ctl release TEST-RGB && expect "list(s['outputs'])" "[]"
bus Library1 Remove asb 1 "$rgbuid" false >/dev/null
echo "rgb checks passed"
