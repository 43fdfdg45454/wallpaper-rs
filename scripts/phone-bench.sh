#!/usr/bin/env bash
set -euo pipefail
usage() {
  cat >&2 <<'EOF'
usage: WPRS_ADB_SERIAL=<serial> scripts/phone-bench.sh build
       WPRS_ADB_SERIAL=<serial> scripts/phone-bench.sh copy FILE.wpak...
       WPRS_ADB_SERIAL=<serial> scripts/phone-bench.sh run NAME.wpak [bench options]
       WPRS_ADB_SERIAL=<serial> scripts/phone-bench.sh clean
Renders a scene package offscreen on a phone over adb, through the renderer of the live wallpaper,
and prints its report as JSON with the battery temperature before and after the run.
Options of run: --level q0|q1|q2|q3|auto|fitted, --frames N, --paced SECONDS, --fps N, --size WxH,
--passes, --shot FILE.png. Environment: ADB (default adb), WPRS_PHONE_DIR (default
/data/local/tmp/wprs-bench), WPRS_COOL (wait until the battery is under this many degrees, default
36), WPRS_AWAKE_TRIES (checks five minutes apart while the screen is on, default 6).
EOF
  exit 2
}
cd "$(dirname "$0")/.."
[ $# -ge 1 ] || usage
: "${WPRS_ADB_SERIAL:?set WPRS_ADB_SERIAL to the serial of the phone (adb devices)}"
ADB=${ADB:-adb}
DIR=${WPRS_PHONE_DIR:-/data/local/tmp/wprs-bench}
COOL=${WPRS_COOL:-36}
TRIES=${WPRS_AWAKE_TRIES:-6}
BIN=${CARGO_TARGET_DIR:-target}/aarch64-linux-android/release/examples/bench
shell() { "$ADB" -s "$WPRS_ADB_SERIAL" shell "$@"; }
send() {
  local size n=0
  size=$(wc -c <"$1")
  "$ADB" -s "$WPRS_ADB_SERIAL" exec-in "cat > '$2'" <"$1"
  until [ "$(shell stat -c %s "$2" | tr -d '\r')" = "$size" ]; do
    n=$((n + 1))
    [ "$n" -le 60 ] || { echo "$2 did not arrive whole" >&2; exit 1; }
    sleep 1
  done
}
tenths() { shell dumpsys battery | sed -n 's/^ *temperature: *\([0-9]*\).*/\1/p' | head -1; }
degrees() { local t; t=$(tenths); printf '%d.%d' "$((t / 10))" "$((t % 10))"; }
thermal() { shell dumpsys thermalservice | sed -n 's/^ *Thermal Status: *\([0-9]*\).*/\1/p' | head -1; }
awake() { shell dumpsys power | grep -q 'mWakefulness=Awake'; }
asleep() {
  local n=0
  while awake; do
    n=$((n + 1))
    [ "$n" -le "$TRIES" ] || { echo "the screen of the phone stayed on, nothing measured" >&2; exit 3; }
    echo "the screen of the phone is on, waiting five minutes ($n of $TRIES)" >&2
    sleep 300
  done
}
cool() {
  local limit
  limit=$(awk -v c="$COOL" 'BEGIN { printf "%d", c * 10 }')
  while [ "$(tenths)" -ge "$limit" ]; do
    echo "battery at $(degrees) °C, waiting under $COOL °C" >&2
    sleep 30
  done
}
case $1 in
  build)
    cargo ndk -t arm64-v8a -P 31 build -p wprs-android --release --locked --example bench
    ;;
  copy)
    shift
    [ -x "$BIN" ] || { echo "build the bench first: $0 build" >&2; exit 1; }
    shell mkdir -p "$DIR/files"
    send "$BIN" "$DIR/bench"
    shell chmod 755 "$DIR/bench"
    for f in "$@"; do send "$f" "$DIR/$(basename "$f")"; done
    shell ls -l "$DIR"
    ;;
  run)
    shift
    [ $# -ge 1 ] || usage
    name=$1
    shift
    asleep
    cool
    before=$(degrees)
    status=$(thermal)
    report=$(shell "cd $DIR && ./bench $DIR/files $DIR/$name $*")
    after=$(degrees)
    later=$(thermal)
    printf '{"battery_c_before": %s, "battery_c_after": %s, "thermal_before": %s, "thermal_after": %s, "report": %s}\n' \
      "$before" "$after" "${status:-null}" "${later:-null}" "$report"
    ;;
  clean)
    shell rm -r "$DIR"
    ;;
  *) usage ;;
esac
