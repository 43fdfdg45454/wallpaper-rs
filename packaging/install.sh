#!/usr/bin/env bash
# Installs wallpaper-rs for the current user, without root:
#   binaries      -> $PREFIX/bin (default ~/.local/bin)
#   systemd unit  -> ~/.config/systemd/user/wprsd.service
#   D-Bus service -> ~/.local/share/dbus-1/services/
#   Plasma plugin -> installed with kpackagetool6 (if available)
#
# Run from the extracted release directory: ./install.sh
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

if command -v kpackagetool6 >/dev/null 2>&1; then
  echo "Installing the Plasma wallpaper plugin"
  if kpackagetool6 --type Plasma/Wallpaper --show io.github.wallpaper_rs >/dev/null 2>&1; then
    kpackagetool6 --type Plasma/Wallpaper --upgrade "$HERE/plasma-wallpaper"
  else
    kpackagetool6 --type Plasma/Wallpaper --install "$HERE/plasma-wallpaper"
  fi
else
  echo "kpackagetool6 not found: skipping the Plasma plugin"
fi

if command -v systemctl >/dev/null 2>&1 && systemctl --user show-environment >/dev/null 2>&1; then
  systemctl --user daemon-reload
  systemctl --user enable wprsd.service
  systemctl --user restart wprsd.service
  echo "wprsd is running: systemctl --user status wprsd"
else
  echo "systemd user session not available: start $BINDIR/wprsd manually"
fi

cat <<MSG

Done. In Plasma: right-click the desktop, "Configure Desktop and Wallpaper",
choose "wallpaper-rs" as the wallpaper type and pick an image.
From a terminal: wprs-ctl set /path/to/image.png
MSG
