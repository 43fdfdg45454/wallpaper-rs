#!/usr/bin/env bash
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
SCENES=$(realpath "${1:-$ROOT/scenes}")
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

synth() {
  python3 - "$1" "$2" <<'PY'
import math, random, struct, sys, wave

kind, out = sys.argv[1], sys.argv[2]
rate, seconds = 24000, 24 if kind == "wind" else 20
n = rate * seconds
rng = random.Random(10 if kind == "wind" else 14)
noise = [rng.uniform(-1, 1) for _ in range(n)]
turn = lambda i, cycles, phase: math.sin(2 * math.pi * cycles * i / n + phase)


def lowpass(signal, coefficient):
    state, out = 0.0, [0.0] * len(signal)
    for _ in range(2):
        for i, x in enumerate(signal):
            state += coefficient(i) * (x - state)
            out[i] = state
    return out


if kind == "wind":
    gust = [0.55 + 0.25 * turn(i, 1, 0.4) + 0.15 * turn(i, 3, 1.3) + 0.05 * turn(i, 7, 2.1) for i in range(n)]
    rumble = lowpass(noise, lambda i: 0.02 + 0.03 * gust[i])
    hiss = lowpass(noise, lambda i: 0.15 + 0.15 * gust[i])
    wide = lowpass(hiss, lambda i: 0.5)
    sound = [(2.5 * rumble[i] + 0.6 * (hiss[i] - 0.6 * wide[i])) * gust[i] for i in range(n)]
else:
    swell = [0.6 + 0.25 * turn(i, 5, 0.2) + 0.15 * turn(i, 2, 1.7) for i in range(n)]
    lap = lowpass(noise, lambda i: 0.05 + 0.05 * swell[i])
    slow = lowpass(lap, lambda i: 0.01)
    sound = [3.0 * (lap[i] - 0.7 * slow[i]) * swell[i] for i in range(n)]
    for _ in range(seconds * 3 // 2):
        start, size = rng.randrange(n), rng.uniform(0.04, 0.12)
        low, high = rng.uniform(380, 520), rng.uniform(900, 1300)
        phase = 0.0
        for k in range(int(rate * 0.09)):
            t = k / rate
            phase += 2 * math.pi * (low + (high - low) * min(1.0, t / 0.04)) / rate
            sound[(start + k) % n] += size * math.sin(phase) * math.exp(-t / 0.025) * min(1.0, t / 0.003)
peak = max(abs(v) for v in sound)
frames = b"".join(struct.pack("<h", int(32767 * 0.6 * v / peak)) for v in sound)
with wave.open(out, "wb") as file:
    file.setnchannels(1)
    file.setsampwidth(2)
    file.setframerate(rate)
    file.writeframes(frames)
PY
}

for pair in snowfall:wind pond:water; do
  scene=${pair%%:*} kind=${pair#*:}
  mkdir -p "$SCENES/$scene/assets"
  synth "$kind" "$TMP/$kind.wav"
  gst-launch-1.0 -q filesrc location="$TMP/$kind.wav" ! wavparse ! audioconvert ! vorbisenc quality=0.0 ! \
    oggmux ! filesink location="$SCENES/$scene/assets/$kind.ogg"
  python3 - "$SCENES/$scene/assets/$kind.ogg" <<'PY'
import struct, sys

path = sys.argv[1]
data, at = bytearray(open(path, "rb").read()), 0
while at < len(data):
    segments = data[at + 26]
    end = at + 27 + segments + sum(data[at + 27 : at + 27 + segments])
    struct.pack_into("<I", data, at + 14, 1)
    struct.pack_into("<I", data, at + 22, 0)
    crc = 0
    for byte in data[at:end]:
        crc ^= byte << 24
        for _ in range(8):
            crc = (crc << 1 ^ 0x04C11DB7 if crc & 1 << 31 else crc << 1) & 0xFFFFFFFF
    struct.pack_into("<I", data, at + 22, crc)
    at = end
open(path, "wb").write(data)
PY
done
