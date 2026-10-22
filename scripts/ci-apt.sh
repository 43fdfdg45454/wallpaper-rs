#!/bin/bash
set -euo pipefail
cache=$HOME/.cache/ci-apt
mkdir -p "$cache/partial"
sudo chown _apt:root "$cache/partial"
sudo rm -f /var/lib/man-db/auto-update
printf 'Acquire::Retries "5";\nAcquire::http::Timeout "30";\nAcquire::https::Timeout "30";\n' |
  sudo tee /etc/apt/apt.conf.d/80ci >/dev/null
sudo apt-get update
sudo DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
  -o Dir::Cache::Archives="$cache" -o APT::Keep-Downloaded-Packages=true "$@"
sudo rm -rf "$cache/partial" "$cache/lock"
sudo chown -R "$(id -u):$(id -g)" "$cache"
