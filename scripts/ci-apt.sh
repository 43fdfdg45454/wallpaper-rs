#!/bin/bash
set -euo pipefail
sudo rm -f /var/lib/man-db/auto-update
printf 'Acquire::Retries "5";\nAcquire::http::Timeout "30";\nAcquire::https::Timeout "30";\n' |
  sudo tee /etc/apt/apt.conf.d/80ci >/dev/null
sudo apt-get update
sudo DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends "$@"
