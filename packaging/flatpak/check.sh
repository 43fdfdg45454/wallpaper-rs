#!/bin/sh
set -eu
bundle=${1:?usage: WPRS_VERSION=<version> $0 <bundle>}
version=${WPRS_VERSION:?}
app=io.github.wallpaper_rs.WallpaperRs
flatpak install -y --noninteractive --bundle "$bundle"
run() { cmd=$1 && shift && flatpak run --env=QT_QPA_PLATFORM=offscreen --command="$cmd" "$@"; }
test "$(run wprs-ctl $app --version)" = "wprs-ctl $version"
test "$(run wprsd $app --version)" = "wprsd $version"
run wprs-app $app --check
files=$(flatpak info -l $app)/files
exports=${files%%/app/*}/exports/share
run wprs-plasma $app > plasma.sh
sh -n plasma.sh
for p in wallpapers/io.github.wallpaper_rs plasmoids/io.github.wallpaper_rs.tray; do
  grep -qF "\"Version\": \"${version%%+*}\"" "$files/share/plasma/$p/metadata.json"
  grep -F -e "--install '$files/share/plasma/$p'" plasma.sh
done
grep "^mkdir " plasma.sh | sh
dir=$(mktemp -d "$HOME/.wprs-check-XXXXXX")
cp "$(dirname "$0")/../../scenes/pond/preview.jpg" "$dir/"
flatpak run --filesystem="$dir:ro" --command=wprs-ctl $app edit set "$dir/preview.jpg" ev=0.5
grep -l '"ev"' "${XDG_DATA_HOME:-$HOME/.local/share}"/wallpaper-rs/edits/*.json
test ! -e "$dir/preview.jpg.wprs.json"
rm -r "$dir"
test -f "$files/share/wallpaper-rs/kwin-pointer/contents/code/main.js"
grep -qF "\"Version\": \"${version%%+*}\"" "$files/share/wallpaper-rs/kwin-pointer/metadata.json"
for name in .Daemon:wprsd :wprs-app; do
  grep -x "Exec=.* --command=/app/bin/${name#*:} $app" "$exports/dbus-1/services/io.github.wallpaper_rs${name%%:*}.service"
done
desktop=$exports/applications/$app.desktop
grep -x "Exec=.* --command=/app/bin/wprs-app --file-forwarding $app @@u %U @@" "$desktop"
grep -x "Icon=$app" "$desktop"
if grep -q '^DBusActivatable=' "$desktop"; then exit 1; fi
test -f "$exports/icons/hicolor/scalable/apps/$app.svg"
test -f "$files/share/metainfo/$app.metainfo.xml"
ls "$files/bin/wprsd" "$files/bin/wprs-ctl" "$files/bin/wprs-app"
if grep -l -E '/run/build|/(home|root)/' "$files"/bin/wprs*; then exit 1; fi
