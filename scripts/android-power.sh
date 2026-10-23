#!/usr/bin/env bash
set -euo pipefail

usage() {
    cat <<'EOF'
Usage: scripts/android-power.sh [-s serial] [-t seconds] [-n runs] [-o file.csv] [scenario...]

Measures the wallpaper on a phone over adb and appends one CSV row per run.

  base      home screen with the same image as a static system wallpaper
  still     home screen with the live wallpaper, untouched
  parallax  swipes between launcher pages every two seconds
  hidden    an app covers the wallpaper
  off       screen off
  saver     parallax with battery saver on
  hot       parallax with a severe thermal status
  tilt      an image that moves with the phone, the phone lying still
  video     home screen with a video wallpaper, untouched
  scene     home screen with the Snowfall example scene, untouched

Without scenarios it runs base, still, parallax, hidden, off, saver, hot, tilt,
video and scene, 30 minutes and 3 runs each. The phone power comes from Perfetto power rails
when trace_processor or trace_processor_shell is on PATH; the app share always
comes from batterystats. extra_mw is the power above the mean of the base rows.
Use adb over Wi-Fi so that the cable does not charge the phone.
EOF
}

package=io.github.wallpaper_rs
seconds=1800 runs=3 out=android-power.csv serial=
while getopts "s:t:n:o:h" opt; do
    case $opt in
        s) serial=$OPTARG ;;
        t) seconds=$OPTARG ;;
        n) runs=$OPTARG ;;
        o) out=$OPTARG ;;
        *) usage; exit 2 ;;
    esac
done
shift $((OPTIND - 1))
(($# > 0)) || set -- base still parallax hidden off saver hot tilt video scene
target=()
[[ -z $serial ]] || target=(-s "$serial")
dev() { adb "${target[@]}" shell "$@" | tr -d '\r'; }
tp=$(command -v trace_processor || command -v trace_processor_shell || true)
source=batterystats
[[ -z $tp ]] || source=perfetto
trace=/data/misc/perfetto-traces/wprs-power.pftrace
local_trace=$(mktemp --suffix=.pftrace)
timeout=$(dev settings get system screen_off_timeout)

restore() {
    dev settings put global low_power 0 >/dev/null || true
    dev cmd thermalservice reset >/dev/null || true
    dev dumpsys battery reset >/dev/null || true
    dev settings put system screen_off_timeout "$timeout" >/dev/null || true
    rm -f "$local_trace"
}
trap restore EXIT

query() { "$tp" -Q "$1" "$local_trace" 2>/dev/null | tail -n 1 | tr -d '"'; }

perfetto_config() {
    cat <<EOF
buffers { size_kb: 65536 }
duration_ms: $((seconds * 1000))
data_sources { config { name: "android.power" android_power_config { battery_poll_ms: 1000 collect_power_rails: true battery_counters: BATTERY_COUNTER_CURRENT battery_counters: BATTERY_COUNTER_VOLTAGE } } }
data_sources { config { name: "android.surfaceflinger.frametimeline" } }
EOF
}

swipes() {
    read -r w h < <(dev wm size | sed -n 's/.*: \([0-9]*\)x\([0-9]*\)$/\1 \2/p' | tail -n 1)
    local left=$((w / 4)) right=$((w * 3 / 4)) y=$((h / 2))
    adb "${target[@]}" shell "while true; do input swipe $right $y $left $y 300; sleep 1;
        input swipe $left $y $right $y 300; sleep 1; done" >/dev/null 2>&1 &
    echo $!
}

home() {
    dev input keyevent KEYCODE_WAKEUP
    dev wm dismiss-keyguard
    dev input keyevent KEYCODE_HOME
}

measure() {
    local scenario=$1 run=$2 mover=
    home
    case $scenario in
        hidden) dev am start -W -a android.settings.SETTINGS >/dev/null ;;
        off) dev input keyevent KEYCODE_SLEEP ;;
        saver) dev settings put global low_power 1 ;;
        hot) dev cmd thermalservice override-status 3 ;;
    esac
    [[ ! $scenario =~ ^(parallax|saver|hot)$ ]] || mover=$(swipes)
    dev dumpsys batterystats --reset >/dev/null
    [[ -z $tp ]] || dev perfetto --txt -c - -o "$trace" --background < <(perfetto_config) >/dev/null
    local layer
    layer=$(dev dumpsys SurfaceFlinger --list | grep -F "$package" | sort | tail -n 1 || true)
    sleep $((seconds - 10))
    [[ -z $layer ]] || dev dumpsys SurfaceFlinger --latency-clear "'$layer'" >/dev/null
    sleep 10
    local device_mw='' fps=0.0 mah uid volts app_mw pss graphics
    [[ -z $layer ]] || fps=$(dev dumpsys SurfaceFlinger --latency "'$layer'" | awk 'NR > 1 && $2 > 0 && $2 < 9e18 {
        n++; if (n == 1 || $2 < lo) lo = $2; if ($2 > hi) hi = $2 }
        END { printf "%.1f", (n >= 127 && hi > lo) ? (n - 1) * 1e9 / (hi - lo) : n / 10 }')
    [[ -z $mover ]] || kill "$mover"
    uid=$(dev dumpsys package "$package" | awk -F 'userId=' 'NF > 1 && u == "" { u = $2 + 0 } END { print u }')
    mah=$(dev dumpsys batterystats --charged "$package" |
        awk -v id="u0a$((uid - 10000)):" 'tolower($1) == "uid" && $2 == id && m == "" { m = $3 } END { print m }')
    volts=$(dev dumpsys battery | awk '$1 == "voltage:" && v == "" { v = $2 / 1000 } END { print v }')
    app_mw=$(awk -v m="${mah:-0}" -v v="$volts" -v s="$seconds" 'BEGIN { printf "%.1f", m * v * 3600 / s }')
    read -r pss graphics < <(dev dumpsys meminfo "$package" |
        awk '$1 == "TOTAL" && $2 == "PSS:" { p = $3 } $1 == "Graphics:" && !g { g = $2 } END { print p + 0, g + 0 }')
    if [[ -n $tp ]]; then
        sleep 5
        adb "${target[@]}" pull "$trace" "$local_trace" >/dev/null
        local rails="select max(value) - min(value) as e from counter join counter_track t on track_id = t.id"
        device_mw=$(query "select round(sum(e) / 1e3 / $seconds, 1) from ($rails where t.name glob 'power.*' group by t.id)")
        [[ -n $device_mw && $device_mw != "[NULL]" ]] || device_mw=$(query "select round(abs(
            (select avg(value) from counter join counter_track t on track_id = t.id where t.name = 'batt.current_ua') *
            (select avg(value) from counter join counter_track t on track_id = t.id where t.name = 'batt.voltage_uv')) / 1e9, 1)")
        fps=$(query "select round(count(*) / $seconds.0, 1) from actual_frame_timeline_slice where layer_name glob '*$package*'")
    fi
    [[ $device_mw != "[NULL]" ]] || device_mw=
    local extra
    extra=$(awk -F, -v d="$device_mw" -v a="$app_mw" '$1 == "base" && $5 != "" { s += $5; n++ }
        END { if (d != "" && n) printf "%.1f", d - s / n; else print a }' "$out")
    echo "$scenario,$run,$seconds,$source,$device_mw,$app_mw,$extra,$fps,$pss,$graphics" | tee -a "$out"
    dev settings put global low_power 0
    dev cmd thermalservice reset
}

[[ -s $out ]] || echo "scenario,run,seconds,source,device_mw,app_mw,extra_mw,fps,pss_kb,graphics_kb" > "$out"
dev dumpsys battery unplug
dev settings put system screen_off_timeout 2147483647
mode=
for scenario in "$@"; do
    wanted=live
    [[ $scenario != base ]] || wanted=static
    [[ ! $scenario =~ ^(video|tilt|scene)$ ]] || wanted=$scenario
    if [[ $mode != "$wanted" && (-n $mode || $wanted != live) ]]; then
        case $wanted in
            static) prompt="Set the same image as a static system wallpaper" ;;
            video) prompt="Choose a 1080p video in wallpaper-rs and set it as the wallpaper" ;;
            tilt) prompt="Turn on Move with the phone for the wallpaper image and lay the phone down" ;;
            scene) prompt="Add the example scenes in wallpaper-rs and set Snowfall as the wallpaper" ;;
            *) prompt="Set an image from wallpaper-rs as the wallpaper, with Move with the phone off" ;;
        esac
        read -r -p "$prompt, then press Enter " _ </dev/tty
    fi
    mode=$wanted
    for run in $(seq "$runs"); do measure "$scenario" "$run"; done
done
