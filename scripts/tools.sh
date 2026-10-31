#!/usr/bin/env bash
set -euo pipefail
name=${1:?usage: $0 actionlint|cosign|syft <dir>}
dir=${2:?usage: $0 actionlint|cosign|syft <dir>}
releases=https://github.com
case $name in
  actionlint) v=1.7.12 sum=8aca8db96f1b94770f1b0d72b6dddcb1ebb8123cb3712530b08cc387b349a3d8
    url=$releases/rhysd/actionlint/releases/download/v$v/actionlint_${v}_linux_amd64.tar.gz ;;
  cosign) v=3.1.3 sum=4629c757b7618056f8ddd7e2625ae9fdd94c0372a65049520bc7d9df9efc7f71
    url=$releases/sigstore/cosign/releases/download/v$v/cosign-linux-amd64 ;;
  syft) v=1.54.1 sum=c069905b391cc4c20a5ba65ad5c10be2a7ba074f8ea6ad203e24d14e303dad47
    url=$releases/anchore/syft/releases/download/v$v/syft_${v}_linux_amd64.tar.gz ;;
  *) echo "unknown tool $name" >&2 && exit 2 ;;
esac
mkdir -p "$dir"
file=$dir/${url##*/}
curl -fsSL --retry 3 -o "$file" "$url"
echo "$sum  $file" | sha256sum -c - >&2
if [[ $file == *.tar.gz ]]; then tar -xzf "$file" -C "$dir" "$name"; else mv "$file" "$dir/$name"; fi
rm -f "$file"
chmod +x "$dir/$name"
echo "$dir/$name"
