#!/usr/bin/env bash
set -euo pipefail
if [ -z "${openrgb:-}" ]; then
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
fi
