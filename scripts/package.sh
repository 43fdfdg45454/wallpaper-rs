#!/usr/bin/env bash
# Builds the release tarball: scripts/package.sh [output-dir]
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
OUT=${1:-$ROOT/dist}
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)
ARCH=$(uname -m)
NAME=wallpaper-rs-$VERSION-$ARCH-linux
STAGE=$(mktemp -d)
trap 'rm -rf "$STAGE"' EXIT

cargo build --release --locked -p wprsd -p wprs-ctl --manifest-path "$ROOT/Cargo.toml"

D=$STAGE/$NAME
mkdir -p "$D/bin" "$D/packaging"
install -m755 "$ROOT/target/release/wprsd" "$ROOT/target/release/wprs-ctl" "$D/bin/"
cp -r "$ROOT/integrations/plasma-wallpaper" "$D/plasma-wallpaper"
cp -r "$ROOT/packaging/systemd" "$ROOT/packaging/dbus" "$D/packaging/"
install -m755 "$ROOT/packaging/install.sh" "$ROOT/packaging/uninstall.sh" "$D/"
cp "$ROOT/LICENSE" "$ROOT/README.md" "$D/"

mkdir -p "$OUT"
tar -C "$STAGE" -czf "$OUT/$NAME.tar.gz" "$NAME"
echo "$OUT/$NAME.tar.gz"
