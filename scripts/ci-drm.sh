#!/usr/bin/env bash
set -euo pipefail
kernel=$(uname -r)
cc=$(grep -oE 'gcc-[0-9]+' /proc/version | head -n 1 || true)

sudo apt-get update -q
sudo apt-get install -y -q "linux-modules-extra-$kernel" || echo "no extra modules package for $kernel"
sudo depmod -a

fetch() {
  local base=https://raw.githubusercontent.com/torvalds/linux/$2/drivers/gpu/drm/$1 files name
  curl -sSfL -o "$3/Makefile" "$base/Makefile"
  files=$(grep -oE '\b[a-z0-9_]+\.o\b' "$3/Makefile" | sed 's/\.o$/.c/' | grep -vx "$1.c" | sort -u)
  while [ -n "$files" ]; do
    for name in $files; do curl -sSfL -o "$3/$name" "$base/$name"; done
    files=$(cat "$3"/*.[ch] | sed -n 's/^#include "\([^"/]*\)"/\1/p' | sort -u \
      | while read -r name; do [ -e "$3/$name" ] || echo "$name"; done)
  done
}

build() {
  local dir=$RUNNER_TEMP/$1 tag
  tag=v$(cut -d. -f1,2 <<<"$kernel")
  sudo apt-get install -y -q "linux-headers-$kernel" ${cc:+"$cc"}
  mkdir -p "$dir"
  fetch "$1" "$tag" "$dir"
  make -s -C "/lib/modules/$kernel/build" M="$dir" CC="${cc:-gcc}" "CONFIG_DRM_${1^^}=m" modules
  sudo install -Dm644 "$dir/$1.ko" "/lib/modules/$kernel/updates/$1.ko"
  sudo depmod -a
  sudo modprobe "$1"
}

for module in vkms vgem; do
  sudo modprobe "$module" || build "$module"
done
sudo udevadm settle
sudo chmod 666 /dev/dri/*
ls -l /dev/dri
