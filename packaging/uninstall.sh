#!/usr/bin/env bash
set -euo pipefail

PREFIX=${PREFIX:-$HOME/.local}
CONFIG_HOME=${XDG_CONFIG_HOME:-$HOME/.config}
DATA_HOME=${XDG_DATA_HOME:-$HOME/.local/share}

if command -v systemctl >/dev/null 2>&1 && systemctl --user show-environment >/dev/null 2>&1; then
  systemctl --user disable --now wprsd.service 2>/dev/null || true
fi
if command -v kpackagetool6 >/dev/null 2>&1; then
  kpackagetool6 --type Plasma/Wallpaper --remove io.github.wallpaper_rs 2>/dev/null || true
  kpackagetool6 --type Plasma/Applet --remove io.github.wallpaper_rs.tray 2>/dev/null || true
fi
rm -f "$PREFIX/bin/wprsd" "$PREFIX/bin/wprs-ctl" "$PREFIX/bin/wprs-app" "$PREFIX/bin/wprs-web" \
  "$CONFIG_HOME/systemd/user/wprsd.service" \
  "$DATA_HOME/dbus-1/services/io.github.wallpaper_rs.Daemon.service" \
  "$DATA_HOME/dbus-1/services/io.github.wallpaper_rs.service" \
  "$DATA_HOME/applications/io.github.wallpaper_rs.desktop" \
  "$DATA_HOME/icons/hicolor/scalable/apps/io.github.wallpaper_rs.svg"
rm -rf "$DATA_HOME/wallpaper-rs/kwin-pointer" "$DATA_HOME/wallpaper-rs/examples"
dbus-send --session --print-reply --dest=org.freedesktop.DBus /org/freedesktop/DBus \
  org.freedesktop.DBus.ReloadConfig >/dev/null 2>&1 || true
if command -v systemctl >/dev/null 2>&1 && systemctl --user show-environment >/dev/null 2>&1; then
  systemctl --user daemon-reload
fi
if [ "${1:-}" = "--purge" ]; then
  rm -rf "$CONFIG_HOME/wallpaper-rs"
fi
echo "wallpaper-rs removed."
