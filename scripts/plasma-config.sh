#!/usr/bin/env bash
set -euo pipefail

activity=6f1d2a0e-3b7c-4e58-9a41-0c2d5e7f8a90
mkdir -p "$HOME/.config"
cat >"$HOME/.config/kactivitymanagerdrc" <<CONF
[activities]
$activity=Default

[main]
currentActivity=$activity
CONF
cat >"$HOME/.config/plasma-org.kde.plasma.desktop-appletsrc" <<CONF
[Containments][1]
activityId=$activity
formfactor=0
immutability=1
lastScreen=0
location=0
plugin=org.kde.plasma.folder
wallpaperplugin=io.github.wallpaper_rs
CONF
updates=$(find /usr/share/plasma/shells/org.kde.plasma.desktop/contents/updates \
  -name '*.js' 2>/dev/null | sort | paste -sd, -)
cat >"$HOME/.config/plasmashellrc" <<CONF
[Updates]
performed=$updates
CONF
