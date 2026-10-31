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

catalog() {
  local domain=$1 locale=$2 pot=$TMP/$1.pot
  sed -i -e '/^#\( \|$\)/d' -e 's/charset=[^\\]*/charset=UTF-8/' "$pot"
  for lang in "${LANGS[@]}"; do
    po=po/$lang/$domain.po
    case $MODE in
      update)
        if [ -f "$po" ]; then
          msgmerge -q -U --backup=none --add-location=file "$po" "$pot"
        else
          mkdir -p "po/$lang"
          msginit -i "$pot" -o "$po" -l "$lang" --no-translator
          sed -i 's/^"\(Last-Translator\|Language-Team\): .*/"\1: wallpaper-rs contributors\\n"/' "$po"
        fi
        msgattrib --no-obsolete -o "$po" "$po"
        sed -i -e 's/^"\(POT-Creation-Date\|PO-Revision-Date\): \([0-9-]*\) .*/"\1: \2 00:00+0000\\n"/' \
          -e '/^"X-Generator: /d' "$po"
        ;;
      check)
        msgfmt --check -o /dev/null "$po"
        msgcmp -N "$po" "$pot"
        if { msgattrib --untranslated --no-obsolete "$po" && msgattrib --only-fuzzy --no-obsolete "$po"; } | grep -q '^msgid '; then
          echo "$po has untranslated or fuzzy strings" >&2 && exit 1
        fi
        ;;
      build) [ -z "$locale" ] || { mkdir -p "$locale/$lang/LC_MESSAGES" && msgfmt --check -o "$locale/$lang/LC_MESSAGES/$domain.mo" "$po"; } ;;
    esac
  done
}

for pkg in integrations/*/; do
  pkg=${pkg%/}
  kind=$(sed -n 's|.*"KPackageStructure": "Plasma/\(.*\)".*|\1|p' "$pkg/metadata.json")
  [ -n "$kind" ] || continue
  domain=plasma_${kind,,}_$(sed -n 's|.*"Id": "\(.*\)".*|\1|p' "$pkg/metadata.json")
  mapfile -t sources < <(find "$pkg/contents" -name '*.qml' | LC_ALL=C sort)
  xgettext --c++ --kde --from-code=UTF-8 -ci18n -k{,x}i18n:1 -k{,x}i18nc:1c,2 -k{,x}i18np:1,2 \
    -k{,x}i18ncp:1c,2,3 -ktr:1 --add-location=file --force-po --package-name=wallpaper-rs -o "$TMP/$domain.pot" "${sources[@]}"
  catalog "$domain" "$OUT/${pkg#integrations/}/contents/locale"
done

mapfile -t sources < <(find apps/wprs-app/qml -name '*.qml' | LC_ALL=C sort)
xgettext --c++ --from-code=UTF-8 -k -ktr:1 --add-location=file --force-po --package-name=wallpaper-rs \
  -o "$TMP/wprs-app.pot" "${sources[@]}"
catalog wprs-app ""
xgettext --c++ --from-code=UTF-8 -k -ktr:1 --add-location=file --force-po --package-name=wallpaper-rs \
  -o "$TMP/wprsd.pot" apps/wprsd/src/words.rs
catalog wprsd ""

mapfile -t sources < <(find crates/wprs-scene/src crates/wprs-format/src crates/wprs-editor/src crates/wprs-core/src/edit -name '*.rs' | LC_ALL=C sort)
for f in "${sources[@]}"; do
  mkdir -p "$TMP/rs/$(dirname "$f")"
  sed -E "s/'([a-z_]+)([^a-z_'])/\2/g" "$f" >"$TMP/rs/$f"
done
sed -i -E 's/^(\s*)\("[a-z_.]+", .*, ("[^"]*")\),?$/\1w(\2);/' "$TMP"/rs/crates/wprs-format/src/{check,validate,web}.rs "$TMP"/rs/crates/wprs-format/src/check/*.rs
sed -i -E 's/^(\s*)\("[a-z_]+", ("[^"]*")\),?$/\1w(\2);/' "$TMP"/rs/crates/wprs-format/src/check.rs "$TMP"/rs/crates/wprs-format/src/check/*.rs
sed -i -E 's/^(\s*)\("[a-z0-9_.]+", ("[^"]*"), \[[A-Za-z, ]+\]\),?$/\1w(\2);/' "$TMP"/rs/crates/wprs-format/src/{features,scenery}.rs
(cd "$TMP/rs" && xgettext --c++ --from-code=UTF-8 -k -ksaid:2 -kw:1 -kknown:2 -kpointed:2 -krequire:3 -kwithin:1 \
  -kUnsupported:2 -kSignature:1 --add-location=file --force-po --package-name=wallpaper-rs -o "$TMP/wprs-messages.pot" "${sources[@]}")
catalog wprs-messages ""
