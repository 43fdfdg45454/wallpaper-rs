#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "$0")"/integration-env.sh "$@"

source "$(dirname "$0")"/integration-video.sh
source "$(dirname "$0")"/integration-streams.sh
source "$(dirname "$0")"/integration-library.sh
source "$(dirname "$0")"/integration-playlists.sh
source "$(dirname "$0")"/integration-repos.sh
source "$(dirname "$0")"/integration-scenes.sh
source "$(dirname "$0")"/integration-content.sh
source "$(dirname "$0")"/integration-inputs.sh
source "$(dirname "$0")"/integration-audio.sh
source "$(dirname "$0")"/integration-sound.sh
source "$(dirname "$0")"/integration-recovery.sh
source "$(dirname "$0")"/integration-sources.sh
source "$(dirname "$0")"/integration-web.sh
source "$(dirname "$0")"/integration-gpu.sh

kill "$daemon"
wait "$daemon" 2>/dev/null || true
WPRS_RENDER_STOP=1 RUST_LOG=$log "$BIN/wprsd" >"$WORK/stopped.log" 2>&1 & daemon=$!; pids+=("$daemon")
wait_for 30 ctl state || fail "the daemon did not start again"
sleep 1
kill -USR2 "$daemon"
guarded() { ! ctl pause 2>"$WORK/guard.out" && grep -q "the renderer stopped after an error; restart wprsd" "$WORK/guard.out"; }
wait_for 10 guarded || fail "a change was accepted without a renderer: $(cat "$WORK/guard.out")"
if ctl set "$ROOT/tests/fixtures/green-64x32.png" 2>/dev/null; then fail "a wallpaper was set without a renderer"; fi
{ ctl lib search "" && ctl playlist ls && ctl provider ls && ctl state; } >/dev/null || fail "the library stopped with the renderer"
kill -0 "$daemon" || fail "the daemon exited with its renderer"
echo "renderer guard checks passed"

status=0
echo "integration test passed"
