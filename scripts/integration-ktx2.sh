#!/usr/bin/env bash
set -euo pipefail
packed=$WORK/packed
mkdir -p "$packed"
ctl scene new "$packed/scene" --id org.example.packed >/dev/null
ctl scene set "$packed/scene" /scene/canvas/size '[64, 32]'
ctl scene set "$packed/scene" /scene/canvas/fit stretch
ctl scene import "$packed/scene" "$ROOT/tests/fixtures/green-64x32.png" --id meadow >/dev/null
ctl scene asset "$packed/scene" meadow --ktx2 --dry-run --json | grep -q '"size":\[64,32\]' || fail "no KTX2 trial"
ctl scene asset "$packed/scene" meadow --ktx2 --dry-run --looks "$WORK/looks" --json | grep -q '"drawn":"plain"' ||
  fail "the KTX2 crops do not say how they were drawn"
[ "$(ctl scene asset "$packed/scene" meadow --ktx2)" = assets/green-64x32.ktx2 ] || fail "the picture did not become KTX2"
ctl scene add-layer "$packed/scene" image --source meadow >/dev/null
ctl scene normals "$packed/scene" meadow --generate --id meadow_normals >/dev/null || fail "no normal map of KTX2"
ctl scene show "$packed/scene" /scene/assets/meadow_normals/path | grep -q 'normals/meadow_normals-1\.ktx2' ||
  fail "the normal map of a KTX2 picture is not KTX2"
ctl scene import "$packed/scene" "$ROOT/tests/fixtures/blink-16x8.gif" --id blink >/dev/null
ctl scene set "$packed/scene" /scene/assets/blink/animated true
[ "$(ctl scene asset "$packed/scene" blink --ktx2)" = assets/blink-16x8.ktx2 ] || fail "the animation did not become KTX2"
ctl scene asset "$packed/scene" blink --json | grep -q '"frames":3' || fail "the KTX2 frames were not told"
said=$(ctl scene asset "$packed/scene" blink)
grep -q "3 frames in a grid" <<<"$said" || fail "the KTX2 frames were not said: $said"
ctl scene import "$packed/scene" "$ROOT/tests/fixtures/green-64x32.png" --id pano >/dev/null
ctl scene asset "$packed/scene" pano --ktx2 --cube --dry-run --json | grep -q '"cube":true' || fail "no cube map trial"
ctl scene asset "$packed/scene" pano --ktx2 --cube --dry-run --json | grep -q '"plain":true' ||
  fail "the classes that keep a cube map uncompressed were not told"
ctl scene asset "$packed/scene" pano --ktx2 --cube >/dev/null || fail "the panorama did not become a cube map"
ctl scene asset "$packed/scene" pano | grep -q "cube map shown as a panorama" || fail "the cube map was not said"
ctl scene import "$packed/scene" "$ROOT/tests/fixtures/blue-8x8.png" --id face >/dev/null
ctl scene asset "$packed/scene" face --ktx2 --faces face,face,face,face,face,face >/dev/null ||
  fail "six faces did not become a cube map"
ctl scene asset "$packed/scene" face --json | grep -q '"cube":true' || fail "the faces were not one cube map"
ctl scene import "$packed/scene" "$ROOT/tests/fixtures/blue-8x8.png" --id few >/dev/null
ctl scene asset "$packed/scene" few --ktx2 --faces few,few 2>/dev/null && fail "two faces became a cube map"
ctl scene build "$packed/scene" -o "$packed/scene.wpak" >/dev/null || fail "the KTX2 scene was not built"
ctl pack check "$packed/scene.wpak" --json | grep -q ktx2 || fail "the KTX2 feature was not listed"
mkdir -p "$packed/picture"
cp "$packed/scene/assets/green-64x32.ktx2" "$packed/picture/image.ktx2"
cat >"$packed/picture/manifest.json" <<'MANIFEST'
{"schema": "wallpaper-rs.wpak/1", "id": "org.example.packedpicture", "version": "1.0.0", "title": "Packed",
 "license": "CC0-1.0", "kind": "image", "entry": "image.ktx2"}
MANIFEST
ctl pack build "$packed/picture" -o "$packed/picture.wpak" >/dev/null || fail "the KTX2 picture was not built"
read -r _ _ _ s12 < <(ctl stream TEST-12 160 90)
uids=()
for file in scene picture; do
  job=$(bus Library1 Import asb 1 "$packed/$file.wpak" false | out "d[0]")
  wait_for 20 finished "$job" || fail "KTX2 $file import: $(bus Daemon1 GetJob t "$job")"
  uid=$(reply "r['job']['output']['imported'][0]['uid']" Daemon1 GetJob t "$job")
  uids+=("$uid")
  ctl lib apply "$uid" -o TEST-12
  shown "$s12" 0 255 0
done
ctl release TEST-12 && expect "list(s['outputs'])" "[]"
bus Library1 Remove asb 2 "${uids[@]}" false >/dev/null
echo "ktx2 checks passed"
