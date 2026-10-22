#!/usr/bin/env bash
set -euo pipefail
kernel=$(uname -r)
cc=$(grep -oE 'gcc-[0-9]+' /proc/version | head -n 1 || true)

sudo apt-get update -q
sudo apt-get install -y -q "linux-modules-extra-$kernel" || echo "no extra modules package for $kernel"
sudo depmod -a

get() {
  local base auth=()
  [ -n "${GITHUB_TOKEN:-}" ] && auth=(-H "Authorization: Bearer $GITHUB_TOKEN")
  for base in "https://raw.githubusercontent.com/torvalds/linux/$2" "https://gitlab.com/linux-kernel/linux/-/raw/$2"; do
    [[ $base == *gitlab* ]] && auth=()
    curl -sSfL --retry 3 --retry-delay 5 --retry-all-errors "${auth[@]}" -o "$1" "$base/$3" && return
  done
  return 1
}

fetch() {
  local dir=drivers/gpu/drm/$1 files name
  get "$3/Makefile" "$2" "$dir/Makefile"
  files=$(grep -oE '\b[a-z0-9_]+\.o\b' "$3/Makefile" | sed 's/\.o$/.c/' | grep -vx "$1.c" | sort -u)
  while [ -n "$files" ]; do
    for name in $files; do get "$3/$name" "$2" "$dir/$name"; done
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
