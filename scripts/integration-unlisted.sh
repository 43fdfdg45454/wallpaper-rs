#!/usr/bin/env bash
set -euo pipefail
declare site
noon=org.example.noon
(
  cd "$site"
  cp -r src noon
  sed -i 's/org.example.dawn/org.example.noon/; s/"Dawn"/"Noon"/' noon/manifest.json
  ctl pack build noon -o noon.wpak >/dev/null
  ctl repo add repo noon.wpak --unlisted >/dev/null
  if ctl repo unlist repo org.example.missing 2>/dev/null; then fail "an unknown package was unlisted"; fi
  if ctl repo unlist repo "$noon" --version 9.0.0 2>/dev/null; then fail "an unknown version was unlisted"; fi
  ctl repo publish repo --key online.key >/dev/null
)
ctl remote refresh --wait >/dev/null
[ "$(ctl pkg search noon --json | jq_ "len(r['packages'])")" = 0 ] || fail "an unlisted package was found"
ctl pkg install "$noon" --wait >/dev/null || fail "an unlisted package did not install by id"
[ "$(ctl pkg search noon --json | jq_ "len(r['packages'])")" = 1 ] || fail "an installed unlisted package is hidden"
ctl pkg uninstall "$noon"
ctl repo list-again "$site/repo" "$noon"
ctl repo publish "$site/repo" --key "$site/online.key" >/dev/null
ctl remote refresh --wait >/dev/null
[ "$(ctl pkg search noon --json | jq_ "len(r['packages'])")" = 1 ] || fail "a listed package was not found"
ctl repo unlist "$site/repo" "$noon" --version 1.0.0
if ctl repo feature "$site/repo" org.example.missing 2>/dev/null; then fail "an unknown package was featured"; fi
if ctl repo feature "$site/repo" "$noon" --note "<b>" 2>/dev/null; then fail "a note with markup was accepted"; fi
ctl repo feature "$site/repo" org.example.dawn --note "Calm mornings"
ctl repo publish "$site/repo" --key "$site/online.key" >/dev/null
ctl remote refresh --wait >/dev/null
[ "$(ctl pkg search sky --json | jq_ "[p['featured'] for p in r['packages']]")" = "[True]" ] ||
  fail "the featured package is not marked: $(ctl pkg search sky --json)"
ctl repo unfeature "$site/repo" org.example.dawn
if ctl repo unfeature "$site/repo" org.example.dawn 2>/dev/null; then fail "a package was unfeatured twice"; fi
echo "unlisted checks passed"
