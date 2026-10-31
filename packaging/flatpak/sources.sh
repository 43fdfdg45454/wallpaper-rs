#!/bin/sh
set -eu
root=$(git -C "$(dirname "$0")" rev-parse --show-toplevel)
out=$root/target/flatpak-src
rm -rf "$out"
mkdir -p "$out"
git -C "$root" ls-files -z | tar -C "$root" --null --no-recursion --ignore-failed-read -T - -cf - | tar -C "$out" -xf -
if [ -n "${WPRS_VERSION:-}" ]; then
  printf '%s\n' "$WPRS_VERSION" > "$out/VERSION"
  sed -i "s/\"Version\": \"[^\"]*\"/\"Version\": \"${WPRS_VERSION%%+*}\"/" "$out"/integrations/*/metadata.json
fi
"$root/packaging/flatpak/cargo-sources.py"
echo "$out"
