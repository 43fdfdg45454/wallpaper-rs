#!/usr/bin/env bash
# Removes everything install.sh installed. User configuration is kept in
# ~/.config/wallpaper-rs unless --purge is given.
set -euo pipefail

PREFIX=${PREFIX:-$HOME/.local}
CONFIG_HOME=${XDG_CONFIG_HOME:-$HOME/.config}
DATA_HOME=${XDG_DATA_HOME:-$HOME/.local/share}

if command -v systemctl >/dev/null 2>&1 && systemctl --user show-environment >/dev/null 2>&1; then
  systemctl --user disable --now wprsd.service 2>/dev/null || true
fi
if command -v kpackagetool6 >/dev/null 2>&1; then
  kpackagetool6 --type Plasma/Wallpaper --remove io.github.wallpaper_rs 2>/dev/null || true
fi
rm -f "$PREFIX/bin/wprsd" "$PREFIX/bin/wprs-ctl" \
  "$CONFIG_HOME/systemd/user/wprsd.service" \
  "$DATA_HOME/dbus-1/services/io.github.wallpaper_rs.Daemon.service"
if command -v systemctl >/dev/null 2>&1 && systemctl --user show-environment >/dev/null 2>&1; then
  systemctl --user daemon-reload
fi
if [ "${1:-}" = "--purge" ]; then
  rm -rf "$CONFIG_HOME/wallpaper-rs"
fi
echo "wallpaper-rs removed."
