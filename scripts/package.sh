#!/usr/bin/env bash
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
OUT=${1:-$ROOT/dist}
export WPRS_VERSION=${WPRS_VERSION:-$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)-local}
VERSION=${WPRS_VERSION%%+*}
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
export CFLAGS="-ffile-prefix-map=$CARGO_HOME=/cargo -ffile-prefix-map=$HOME=/build -ffile-prefix-map=$ROOT=/src"
export CXXFLAGS=$CFLAGS
cargo build --release --locked -p wprsd -p wprs-ctl -p wprs-app --manifest-path "$ROOT/Cargo.toml"

SRC=$STAGE/src
D=$STAGE/$NAME
mkdir -p "$SRC" "$D/bin" "$D/packaging"
git -C "$ROOT" archive HEAD integrations packaging scenes LICENSE README.md | tar -x -C "$SRC"
for bin in wprsd wprs-ctl wprs-app; do
  install -m755 "$ROOT/target/release/$bin" "$D/bin/"
  objcopy --remove-section=.comment "$D/bin/$bin"
done
cp -r "$SRC/integrations/plasma-wallpaper" "$D/plasma-wallpaper"
cp -r "$SRC/integrations/plasma-tray" "$D/plasma-tray"
cp -r "$SRC/integrations/kwin-pointer" "$D/kwin-pointer"
sed -i "s/\"Version\": \"[^\"]*\"/\"Version\": \"$VERSION\"/" "$D"/*/metadata.json
"$ROOT/scripts/translations.sh" build "$D"
mkdir -p "$D/examples"
for scene in "$SRC"/scenes/*/; do
  "$ROOT/target/release/wprs-ctl" pack build "$scene" -o "$D/examples/$(basename "$scene").wpak" >/dev/null
done
cp "$SRC/scenes/LICENSE" "$D/examples/"
cp -r "$SRC/packaging/systemd" "$SRC/packaging/dbus" "$SRC/packaging/desktop" "$SRC/packaging/icons" "$D/packaging/"
install -m755 "$SRC/packaging/install.sh" "$SRC/packaging/uninstall.sh" "$D/"
cp "$SRC/LICENSE" "$SRC/README.md" "$D/"

mkdir -p "$OUT"
tar -C "$STAGE" --sort=name --owner=0 --group=0 --numeric-owner --mtime="@$SOURCE_DATE_EPOCH" \
  --pax-option=exthdr.name=%d/PaxHeaders/%f,delete=atime,delete=ctime -cf - "$NAME" | gzip -n -9 > "$OUT/$NAME.tar.gz"
echo "$OUT/$NAME.tar.gz"
