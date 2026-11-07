#!/usr/bin/env bash
set -euo pipefail
python3 - "$WORK/tools.png" "$WORK/tools-depth.png" <<'PNG'
import struct, sys, zlib
def chunk(kind, data):
    return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))
def save(path, kind, pixel):
    rows = b"".join(b"\0" + b"".join(pixel(x, y) for x in range(96)) for y in range(64))
    with open(path, "wb") as out:
        out.write(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", 96, 64, 8, kind, 0, 0, 0)))
        out.write(chunk(b"IDAT", zlib.compress(rows)) + chunk(b"IEND", b""))
near = lambda x, y: 32 <= x < 48 and 20 <= y < 44
mid = lambda x, y: 60 <= x < 80 and 10 <= y < 50
save(sys.argv[1], 2, lambda x, y: bytes((230, 40, 40) if near(x, y) else (40, 160, 60) if not mid(x, y) else (40, 60, 200)))
save(sys.argv[2], 0, lambda x, y: bytes([230 if near(x, y) else 130 if mid(x, y) else 30]))
PNG
tools=$WORK/tools.png
ctl edit depth import "$tools" "$WORK/tools-depth.png" >/dev/null
plain=$(ctl edit depth layers "$tools" --fill simple --layers 4) || fail "the generated picture has no layers"
sets() { ctl edit depth layers "$tools" --list | grep -c "layers, fill simple" || true; }
ctl edit depth brush "$tools" "0.1,0.1;0.2,0.1" --raise --by 0.5 --radius 0.1 >/dev/null
ctl edit depth cuts "$tools" 0.3,0.7 >/dev/null
ctl edit depth fill-mask "$tools" "0.42,0.5" --refill --radius 0.05 >/dev/null
ctl edit depth fill-mask "$tools" "0.9,0.9" --protect --radius 0.05 >/dev/null
edited=$(ctl edit depth layers "$tools" --fill simple --layers 4) || fail "the edited depth has no layers"
[ "$edited" != "$plain" ] || fail "strokes, cuts and marks did not change the layers"
ctl edit depth layers "$tools" --list | grep -q "$(basename "$edited").*3 layers" || fail "the cuts did not make 3 layers: $(ctl edit depth layers "$tools" --list)"
ctl edit depth show "$tools" --layer 1 --out "$WORK/tools-layer.png" --band "$WORK/tools-band.png" >/dev/null
for file in tools-layer tools-band; do [ -s "$WORK/$file.png" ] || fail "$file was not saved"; done
ctl edit depth show "$tools" --layer 0 --out "$WORK/tools-far.png" --fill simple >/dev/null
if ctl edit depth show "$tools" --layer 0 --out "$WORK/x.png" --fill lama 2>/dev/null; then fail "a set that was never built was shown"; fi
ctl edit depth export "$tools" "$WORK/tools-seen.png" --adjusted >/dev/null
for _ in 1 2 3 4; do ctl edit undo "$tools"; done
[ "$(ctl edit depth layers "$tools" --fill simple --layers 4)" = "$plain" ] || fail "undo did not bring the first layers back"
[ "$(sets)" = 2 ] || fail "$(sets) layer sets after the strokes, expected 2"
ctl edit depth layers "$tools" --clear >/dev/null
echo "depth tool checks passed"
