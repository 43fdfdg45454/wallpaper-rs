#!/usr/bin/env bash
set -euo pipefail
declare scene ROOT
maker=$scene/maker
ctl scene new "$maker" --id org.example.maker >/dev/null
ctl scene import "$maker" "$ROOT/tests/fixtures/green-64x32.png" --id leaf >/dev/null
ctl scene add-layer "$maker" image --id sprout >/dev/null
ctl scene add-layer "$maker" particles --id dust >/dev/null
if ctl pack new-assets "$maker" "$scene/bad-parts" --part sound:x --id org.example.parts --title Parts \
  --license CC0-1.0 --attribution Example 2>/dev/null; then fail "an unknown kind of part was offered"; fi
ctl pack new-assets "$maker" "$scene/parts" --part layer:sprout --part particles:dust --id org.example.parts \
  --title Parts --license CC-BY-4.0 --attribution "Example Maker" --part-title "layer:sprout=Sprout" \
  --part-thumbnail "layer:sprout=$ROOT/tests/fixtures/green-64x32.png" >/dev/null || fail "no assets package was made"
ctl pack build "$scene/parts" -o "$scene/parts.wpak" >/dev/null || fail "the assets package did not build"
ctl pack parts "$scene/parts.wpak" | grep -q 'layer:sprout "Sprout"' || fail "the parts were not listed"
ctl pack parts "$scene/parts.wpak" | grep -q 'CC-BY-4.0 Example Maker' || fail "the license of a file was not listed"
user=$scene/user
ctl scene new "$user" --id org.example.user >/dev/null
if ctl scene import-assets "$user" "$scene/parts.wpak" --part layer:gone 2>/dev/null; then
  fail "a part the package does not offer was imported"
fi
ctl scene import-assets "$user" "$scene/parts.wpak" --part layer:sprout >/dev/null || fail "no part was imported"
ctl scene show "$user" /manifest/credits | grep -q '"id": "org.example.parts"' || fail "the import was not credited"
ctl scene show "$user" /manifest/origin | grep -q '"derived"' || fail "the origin does not name the parts"
ctl scene validate "$user" >/dev/null || fail "the imported part left the scene invalid"
ctl scene undo "$user" >/dev/null
if ctl scene show "$user" /manifest/credits >/dev/null 2>&1; then fail "undo kept the credits of the import"; fi
