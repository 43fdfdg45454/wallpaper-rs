#!/usr/bin/env bash
set -euo pipefail
TARBALL=$(realpath "${1:?usage: $0 <release-tarball> [output-dir]}")
OUT=$(realpath -m "${2:-dist}")
ROOT=$(cd "$(dirname "$0")/.." && pwd)
VERSION=$(basename "$TARBALL" | sed -n 's/^wallpaper-rs-\(.*\)-x86_64-linux\.tar\.gz$/\1/p')
TOP=$(mktemp -d)
trap 'rm -rf "$TOP"' EXIT
mkdir -p "$TOP/SOURCES" "$OUT"
cp "$TARBALL" "$TOP/SOURCES/"
rpmbuild -bb --define "_topdir $TOP" --define "wprs_version $VERSION" "$ROOT/packaging/rpm/wallpaper-rs.spec"
find "$TOP/RPMS" -name '*.rpm' -exec cp {} "$OUT/" \;
ls "$OUT"/*.rpm
