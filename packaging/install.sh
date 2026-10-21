#!/usr/bin/env bash
set -euo pipefail

HERE=$(cd "$(dirname "$0")" && pwd)
PREFIX=${PREFIX:-$HOME/.local}
BINDIR=$PREFIX/bin
CONFIG_HOME=${XDG_CONFIG_HOME:-$HOME/.config}
DATA_HOME=${XDG_DATA_HOME:-$HOME/.local/share}
UNIT_DIR=$CONFIG_HOME/systemd/user
DBUS_DIR=$DATA_HOME/dbus-1/services

echo "Installing binaries to $BINDIR"
install -Dm755 "$HERE/bin/wprsd" "$BINDIR/wprsd"
install -Dm755 "$HERE/bin/wprs-ctl" "$BINDIR/wprs-ctl"

echo "Installing the user service"
mkdir -p "$UNIT_DIR" "$DBUS_DIR"
sed "s|@BINDIR@|$BINDIR|g" "$HERE/packaging/systemd/wprsd.service" >"$UNIT_DIR/wprsd.service"
sed "s|@BINDIR@|$BINDIR|g" "$HERE/packaging/dbus/io.github.wallpaper_rs.Daemon.service" \
  >"$DBUS_DIR/io.github.wallpaper_rs.Daemon.service"

install_package() {
  if kpackagetool6 --type "$1" --show "$2" >/dev/null 2>&1; then
    kpackagetool6 --type "$1" --upgrade "$3"
    plugin_upgraded=1
  else
    kpackagetool6 --type "$1" --install "$3"
  fi
}

if command -v kpackagetool6 >/dev/null 2>&1; then
  echo "Installing the Plasma wallpaper plugin and tray icon"
  install_package Plasma/Wallpaper io.github.wallpaper_rs "$HERE/plasma-wallpaper"
  install_package Plasma/Applet io.github.wallpaper_rs.tray "$HERE/plasma-tray"
else
  echo "kpackagetool6 not found: skipping the Plasma plugin and tray icon"
fi

if command -v systemctl >/dev/null 2>&1 && systemctl --user show-environment >/dev/null 2>&1; then
  systemctl --user daemon-reload
  systemctl --user enable wprsd.service
  systemctl --user restart wprsd.service
  echo "wprsd is running: systemctl --user status wprsd"
else
  echo "systemd user session not available: start $BINDIR/wprsd manually"
fi

if [ "${plugin_upgraded:-0}" = 1 ]; then
  cat <<MSG

The Plasma plugins were upgraded. Plasma keeps the old versions in memory until
it restarts: log out and back in, or run
  systemctl --user restart plasma-plasmashell
MSG
fi

cat <<MSG

Done. In Plasma: right-click the desktop, "Configure Desktop and Wallpaper" and
choose "wallpaper-rs" as the wallpaper type. Manage it from the wallpaper-rs icon
in the system tray (after restarting Plasma) or from a terminal:
  wprs-ctl set /path/to/image-or-video
  wprs-ctl --help
MSG
