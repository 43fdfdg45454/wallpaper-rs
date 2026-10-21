#!/usr/bin/env bash
# Plasma end-to-end test. Runs as root in a disposable container that has
# Plasma 6 installed: creates an unprivileged user, installs the release
# tarball with install.sh, starts a headless KWin + plasmashell session,
# selects the wallpaper-rs plugin, takes a screenshot and checks its pixels.
#
# Usage: scripts/plasma-test.sh <release-tarball> <output-dir>
set -euo pipefail

TARBALL=$(realpath "${1:?usage: $0 <release-tarball> <output-dir>}")
OUT=$(mkdir -p "${2:?usage: $0 <release-tarball> <output-dir>}" && realpath "$2")
ROOT=$(cd "$(dirname "$0")/.." && pwd)
TEST_USER=plasmatest

id "$TEST_USER" >/dev/null 2>&1 || useradd -m "$TEST_USER"
HOME_DIR=$(getent passwd "$TEST_USER" | cut -d: -f6)
WORK=$HOME_DIR/test
rm -rf "$WORK"
mkdir -p "$WORK"
cp "$TARBALL" "$WORK/release.tar.gz"
cp "$ROOT/tests/fixtures/green-64x32.png" "$WORK/green.png"
cp "$ROOT/scripts/plasma-session.sh" "$WORK/plasma-session.sh"
chown -R "$TEST_USER:" "$WORK"
chmod 777 "$OUT"

status=0
su -l "$TEST_USER" -s /bin/bash -c "bash '$WORK/plasma-session.sh' '$WORK' '$OUT'" || status=$?
exit "$status"
