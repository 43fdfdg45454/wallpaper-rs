#!/usr/bin/env bash
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
OUT=${1:-$ROOT/dist}
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)
ARCH=$(uname -m)
NAME=wallpaper-rs-$VERSION-$ARCH-linux
STAGE=$(mktemp -d)
trap 'rm -rf "$STAGE"' EXIT

export SOURCE_DATE_EPOCH=${SOURCE_DATE_EPOCH:-$(git -C "$ROOT" log -1 --format=%ct 2>/dev/null || echo 0)}
CARGO_HOME=${CARGO_HOME:-$HOME/.cargo}
RUSTUP_HOME=${RUSTUP_HOME:-$HOME/.rustup}
unset CARGO_ENCODED_RUSTFLAGS CARGO_BUILD_RUSTFLAGS
export RUSTFLAGS="--remap-path-prefix=$CARGO_HOME=/cargo --remap-path-prefix=$RUSTUP_HOME=/rustup \
  --remap-path-prefix=$HOME=/build --remap-path-prefix=$ROOT=/src"
cargo build --release --locked -p wprsd -p wprs-ctl --manifest-path "$ROOT/Cargo.toml"

SRC=$STAGE/src
D=$STAGE/$NAME
mkdir -p "$SRC" "$D/bin" "$D/packaging"
git -C "$ROOT" archive HEAD integrations packaging LICENSE README.md | tar -x -C "$SRC"
install -m755 "$ROOT/target/release/wprsd" "$ROOT/target/release/wprs-ctl" "$D/bin/"
objcopy --remove-section=.comment "$D/bin/wprsd"
objcopy --remove-section=.comment "$D/bin/wprs-ctl"
cp -r "$SRC/integrations/plasma-wallpaper" "$D/plasma-wallpaper"
cp -r "$SRC/integrations/plasma-tray" "$D/plasma-tray"
"$ROOT/scripts/translations.sh" build "$D"
cp -r "$SRC/packaging/systemd" "$SRC/packaging/dbus" "$D/packaging/"
install -m755 "$SRC/packaging/install.sh" "$SRC/packaging/uninstall.sh" "$D/"
cp "$SRC/LICENSE" "$SRC/README.md" "$D/"

mkdir -p "$OUT"
tar -C "$STAGE" --sort=name --owner=0 --group=0 --numeric-owner --mtime="@$SOURCE_DATE_EPOCH" \
  --pax-option=exthdr.name=%d/PaxHeaders/%f,delete=atime,delete=ctime -cf - "$NAME" | gzip -n -9 > "$OUT/$NAME.tar.gz"
echo "$OUT/$NAME.tar.gz"
