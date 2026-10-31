#!/bin/sh
set -eu
file=${1:?usage: $0 <metainfo> <version> [git-dir]}
version=${2%%+*}
root=${3:-.}
g() { git -c safe.directory="$root" -C "$root" "$@" 2>/dev/null; }
esc() { sed -e 's/&/\&amp;/g' -e 's/</\&lt;/g' -e 's/>/\&gt;/g' -e 's/"/\&quot;/g'; }
day=$(g log -1 --format=%cs HEAD || date -u +%F)
prev=$(g describe --tags --abbrev=0 --match 'v[0-9]*' HEAD || true)
notes=$(g log --no-merges -E --format=%s ${prev:+"$prev..HEAD"} |
  sed -n -E 's/^(feat|fix|perf)(\([^()]+\))?!?: (.+)$/\3/p' | head -n 30 | esc || true)
older=$(g tag --list 'v[0-9]*' --sort=-v:refname | grep -E '^v[0-9]+\.[0-9]+\.[0-9]+$' | grep -vxF "v$version" | head -n 10 || true)
block=$(mktemp)
trap 'rm -f "$block"' EXIT
{
  echo '  <releases>'
  if [ -n "$notes" ]; then
    printf '    <release version="%s" date="%s">\n      <description>\n        <ul>\n' "$version" "$day"
    printf '%s\n' "$notes" | sed 's|.*|          <li>&</li>|'
    printf '        </ul>\n      </description>\n    </release>\n'
  else
    printf '    <release version="%s" date="%s"/>\n' "$version" "$day"
  fi
  for tag in $older; do
    printf '    <release version="%s" date="%s"/>\n' "${tag#v}" "$(g log -1 --format=%cs "$tag")"
  done
  echo '  </releases>'
} > "$block"
sed -i -e '/^  <releases>$/,/^  <\/releases>$/d' "$file"
sed -i -e "/^<\/component>$/{
r $block
d
}" "$file"
echo '</component>' >> "$file"
