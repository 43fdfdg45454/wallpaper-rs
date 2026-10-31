#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "$0")"/integration-env.sh "$@"
RUST_LOG=$log "$BIN/wprsd" >"$WORK/wprsd.log" 2>&1 & daemon=$!; pids+=("$daemon")
wait_for 30 ctl state || fail "the daemon did not start"
source "$(dirname "$0")"/integration-web.sh
kill "$daemon"
status=0
echo "web test passed"
