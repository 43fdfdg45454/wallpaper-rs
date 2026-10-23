#!/usr/bin/env bash
set -euo pipefail
IN=${1:?usage: VERSION=<semver> $0 <artifacts-dir>}
TAG=v${VERSION:?}
SEMVER='^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$'
[[ $VERSION =~ $SEMVER ]] || { echo "$VERSION is not a release version" >&2; exit 1; }

TIP=$(gh api "repos/{owner}/{repo}/commits/master" --jq .sha)
[ "$TIP" = "$(git rev-parse HEAD)" ] || { echo "HEAD is not the tip of master, so nothing is published"; exit 0; }
TOP=$(gh api "repos/{owner}/{repo}/git/matching-refs/tags/v" --paginate --jq '.[].ref | ltrimstr("refs/tags/v")' |
  { grep -E "$SEMVER" || true; } | sort -V | tail -1)
printf '%s\n' "$TOP" "$VERSION" | sort -VCu || { echo "$TAG is not newer than v$TOP, so nothing is published"; exit 0; }

OUT=$(mktemp -d)
trap 'rm -rf "$OUT"' EXIT
mkdir "$OUT/files"
cp "$IN"/wallpaper-rs-*-x86_64-linux.tar.gz "$OUT/files/wallpaper-rs-x86_64-linux.tar.gz"
cp "$IN/wallpaper-rs.flatpak" "$OUT/files/wallpaper-rs.flatpak"
cp "$IN"/wallpaper-rs-[0-9]*.x86_64.rpm "$OUT/files/wallpaper-rs.x86_64.rpm"
cp "$IN"/wallpaper-rs-examples-*.noarch.rpm "$OUT/files/wallpaper-rs-examples.noarch.rpm"
cp "$IN/wallpaper-rs-android.apk" "$OUT/files/wallpaper-rs-android.apk"
(cd "$OUT/files" && sha256sum -- * > SHA256SUMS)

PREV=$(git describe --tags --abbrev=0 --match 'v[0-9]*' HEAD 2>/dev/null || true)
TYPE='(build|chore|ci|docs|feat|fix|perf|refactor|revert|style|test)'
SCOPE='(\([^()]+\))?'
FOOTER='^BREAKING[ -]CHANGE: '
log() { git log --no-merges -E --format=%s "$@" "${PREV:+$PREV..}HEAD"; }
pick() { grep -E "$1" | sed 's/^/- /' || true; }
section() { [ -z "$2" ] || printf '## %s\n\n%s\n\n' "$1" "$2"; }
{
  section "Breaking changes" "$(log | pick "^$TYPE$SCOPE!: "; log --grep="$FOOTER" | pick "^$TYPE$SCOPE: ")"
  section Features "$(log --invert-grep --grep="$FOOTER" | pick "^feat$SCOPE: ")"
  section Fixes "$(log --invert-grep --grep="$FOOTER" | pick "^(fix|perf)$SCOPE: ")"
  echo "Verify the files with: sha256sum -c SHA256SUMS"
} > "$OUT/notes"
gh release create "$TAG" "$OUT"/files/* --target "$(git rev-parse HEAD)" --title "$VERSION" --notes-file "$OUT/notes" --latest

if gh release view latest-build >/dev/null 2>&1; then
  gh release delete latest-build --cleanup-tag --yes
elif git rev-parse -q --verify refs/tags/latest-build >/dev/null; then
  gh api -X DELETE "repos/{owner}/{repo}/git/refs/tags/latest-build"
fi
