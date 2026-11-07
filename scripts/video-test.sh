#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "$0")"/integration-env.sh "$@"
source "$(dirname "$0")"/integration-video.sh
status=0
echo "video test passed"
