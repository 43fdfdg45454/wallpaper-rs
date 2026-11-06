#!/usr/bin/env bash
set -euo pipefail
declare pages web tint
ctl pack build "$ROOT/tests/fixtures/pulse" -o "$pages/pulse.wpak" >/dev/null && ctl lib import "$pages/pulse.wpak" --wait >/dev/null
"$BIN/probe" "$web" 1000 --hold >/dev/null 2>&1 & pacer=$!; pids+=("$pacer")
ctl config playback.max_fps 10
ctl lib apply "$(uid org.example.pulse-page)" -o TEST-W 2>/dev/null
paged() { field "s['outputs']['TEST-W']['page_frames']"; }
wait_for 20 sh -c "[ \"\$('$BIN/wprs-ctl' state | python3 -c 'import json, sys; print(json.load(sys.stdin)[\"outputs\"][\"TEST-W\"][\"page_frames\"])')\" -gt 2 ]" ||
  fail "the animated page drew nothing: $(ctl state)"
before=$(paged) && sleep 3 && drawn=$(($(paged) - before))
if [ "$drawn" -lt 6 ] || [ "$drawn" -gt 45 ]; then fail "a page capped at 10 frames a second drew $drawn frames in 3 seconds"; fi
ctl config playback.max_fps display
kill "$pacer" 2>/dev/null || true; wait "$pacer" 2>/dev/null || true
ctl lib apply "$tint" -o TEST-W 2>/dev/null
echo "a page draws no more frames than its cap ($drawn in 3 s at 10)"
