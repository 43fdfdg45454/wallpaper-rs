#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
dbus_interface=apps/wprsd/src/service.rs
long=$(git ls-files -z '*.rs' '*.qml' '*.kt' '*.sh' '*.js' ':!:vendor/' | xargs -0 wc -l |
  awk -v whole="$dbus_interface" '$2 != "total" && $2 != whole && $1 > 200')
if [ -n "$long" ]; then
  echo "files over 200 lines:" >&2
  echo "$long" >&2
  exit 1
fi
