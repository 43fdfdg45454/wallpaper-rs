#!/usr/bin/env bash
set -euo pipefail
APK=$(realpath "${1:?usage: [WANT="<abi> ..."] ANDROID_HOME=<sdk> ANDROID_NDK_HOME=<ndk> $0 <release.apk>}")
NDK=${ANDROID_NDK_HOME:-${ANDROID_NDK_LATEST_HOME:?ANDROID_NDK_HOME is not set}}
ROOT=$(cd "$(dirname "$0")/.." && pwd)
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT
fail() { echo "::error::$*"; exit 1; }
readelf=$(find "$NDK/toolchains/llvm/prebuilt" -path '*/bin/llvm-readelf' -print -quit)
zipalign=$(find "${ANDROID_HOME:?ANDROID_HOME is not set}/build-tools" -mindepth 2 -maxdepth 2 -name zipalign | sort -V | tail -n 1)
unzip -q "$APK" 'lib/*' -d "$WORK"
abis=$(find "$WORK/lib" -mindepth 1 -maxdepth 1 -type d -printf '%f\n' | sort | tr '\n' ' ')
[ -n "${WANT:-}" ] || WANT="arm64-v8a x86_64 "
[ "$abis" = "$WANT" ] || fail "the APK holds the architectures $abis instead of $WANT"
for abi in $abis; do
  test -f "$WORK/lib/$abi/libwprs_android.so" || fail "the APK has no lib/$abi/libwprs_android.so"
done
if find "$WORK/lib" -name 'libonnxruntime*' | grep -q .; then
  fail "the APK carries ONNX Runtime, which is downloaded with the depth model"
fi
if grep -a -l -F -e "$HOME/" -e "$ROOT/" "$WORK"/lib/*/libwprs_android.so; then
  fail "the native library keeps paths of the build machine"
fi
for lib in "$WORK"/lib/*/*.so; do
  aligns=$("$readelf" -lW "$lib" | awk '$1 == "LOAD" { print $NF }')
  [ -n "$aligns" ] || fail "${lib#"$WORK/"} has no LOAD segment"
  for align in $aligns; do
    ((align >= 0x4000)) || fail "${lib#"$WORK/"} has a LOAD segment aligned to $align"
  done
done
"$zipalign" -c -P 16 4 "$APK" || fail "the APK is not aligned for 16 KB pages"
echo "APK checks passed: native libraries, 16 KB alignment and no build paths"
