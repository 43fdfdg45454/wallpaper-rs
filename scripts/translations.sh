#!/usr/bin/env bash
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
MODE=${1:-}
OUT=$(realpath -m "${2:-$ROOT/integrations}")
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
case $MODE in
  update | check | build) ;;
  *) echo "usage: $0 update [language...] | check | build [output-dir]" >&2 && exit 2 ;;
esac
cd "$ROOT"
shopt -s nullglob
LANGS=(po/*/)
LANGS=("${LANGS[@]#po/}")
LANGS=("${LANGS[@]%/}")
if [ "$MODE" = update ]; then LANGS+=("${@:2}"); fi

for pkg in integrations/*/; do
  pkg=${pkg%/}
  kind=$(sed -n 's|.*"KPackageStructure": "Plasma/\(.*\)".*|\1|p' "$pkg/metadata.json")
  domain=plasma_${kind,,}_$(sed -n 's|.*"Id": "\(.*\)".*|\1|p' "$pkg/metadata.json")
  pot=$TMP/$domain.pot
  mapfile -t sources < <(find "$pkg/contents" -name '*.qml' | LC_ALL=C sort)
  xgettext --c++ --kde --from-code=UTF-8 -ci18n -k{,x}i18n:1 -k{,x}i18nc:1c,2 -k{,x}i18np:1,2 \
    -k{,x}i18ncp:1c,2,3 --add-location=file --force-po --package-name=wallpaper-rs -o "$pot" "${sources[@]}"
  sed -i -e '/^#\( \|$\)/d' -e 's/charset=[^\\]*/charset=UTF-8/' "$pot"
  for lang in "${LANGS[@]}"; do
    po=po/$lang/$domain.po
    mo=$OUT/${pkg#integrations/}/contents/locale/$lang/LC_MESSAGES/$domain.mo
    case $MODE in
      update)
        if [ -f "$po" ]; then
          msgmerge -q -U --backup=none --add-location=file "$po" "$pot"
        else
          mkdir -p "po/$lang"
          msginit -i "$pot" -o "$po" -l "$lang" --no-translator
          sed -i 's/^"\(Last-Translator\|Language-Team\): .*/"\1: wallpaper-rs contributors\\n"/' "$po"
        fi
        sed -i -e 's/^"\(POT-Creation-Date\|PO-Revision-Date\): \([0-9-]*\) .*/"\1: \2 00:00+0000\\n"/' \
          -e '/^"X-Generator: /d' "$po"
        ;;
      check)
        msgfmt --check -o /dev/null "$po"
        msgcmp -N "$po" "$pot"
        ;;
      build) mkdir -p "$(dirname "$mo")" && msgfmt --check -o "$mo" "$po" ;;
    esac
  done
done
