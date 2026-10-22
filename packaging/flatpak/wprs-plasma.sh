#!/bin/sh
set -eu
mode=${1:-install}
case $mode in
  install | remove) ;;
  *) echo "usage: wprs-plasma [install | remove] | sh" >&2 && exit 2 ;;
esac
files=$(sed -n 's/^app-path=//p' /.flatpak-info)
test -n "$files"
for pkg in Wallpaper:wallpapers/io.github.wallpaper_rs Applet:plasmoids/io.github.wallpaper_rs.tray; do
  type=Plasma/${pkg%%:*}
  dir=$files/share/plasma/${pkg#*:}
  id=${dir##*/}
  if [ "$mode" = remove ]; then
    echo "kpackagetool6 --type $type --remove $id"
  else
    echo "if kpackagetool6 --type $type --show $id >/dev/null 2>&1; then" \
      "kpackagetool6 --type $type --upgrade '$dir'; else kpackagetool6 --type $type --install '$dir'; fi"
  fi
done
if [ "$mode" = install ]; then
  echo "Then restart Plasma (systemctl --user restart plasma-plasmashell) and choose wallpaper-rs" \
    "in \"Configure Desktop and Wallpaper\"." >&2
fi
