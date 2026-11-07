#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
URL=http://127.0.0.1:8080
PASSWORD=correct-horse-battery

id=$(docker compose ps -q wprs)
test "$(docker inspect --format '{{.State.Health.Status}}' "$id")" = healthy
test "$(docker inspect --format '{{.HostConfig.ReadonlyRootfs}}' "$id")" = true
pid=$(docker inspect --format '{{.State.Pid}}' "$id")
test "$(awk '/^(Uid|Gid):/ {print $2, $3, $4, $5}' "/proc/$pid/status" | sort -u)" = "65532 65532 65532 65532"
test "$(awk '/^CapEff:/ {print $2}' "/proc/$pid/status")" = 0000000000000000
if docker compose exec -T wprs sh -c true 2>/dev/null; then
  echo "the image must not contain a shell" >&2
  exit 1
fi

test "$(curl -fsS "$URL/healthz")" = ok
curl -fsS "$URL/.well-known/wprs-server" | grep '"end_to_end_default":true' > /dev/null
[ -z "${WPRS_VERSION:-}" ] || curl -fsS "$URL/.well-known/wprs-server" | grep -F "\"version\":\"$WPRS_VERSION\"" > /dev/null
docker compose logs wprs | grep 'one-time setup token' > /dev/null

token=$(docker compose exec -T wprs wprs-server setup-token)
status=$(curl -s -o /dev/null -w '%{http_code}' --data-urlencode "token=$token" -d handle=admin -d "password=$PASSWORD" "$URL/setup")
test "$status" = 303

docker compose exec -T wprs wprs-server repo init --name "Smoke test" | grep 'Root key fingerprint' > /dev/null
docker compose exec -T wprs wprs-server repo publish
latest=$(curl -fsS "$URL/repo/entry/latest")
curl -fsS "$URL/repo/entry/$latest.json.minisig" > /dev/null
curl -fsS "$URL/repo/entry/$latest.json" > /dev/null

docker compose exec -T wprs wprs-server backup /data/backups/smoke
docker compose stop wprs
docker compose run --rm -T wprs restore /data/backups/smoke --force
docker compose up -d --wait wprs
test "$(curl -fsS "$URL/repo/entry/latest")" -gt "$latest"

login='{"handle":"admin","password":"'"$PASSWORD"'","device":{"platform":"linux"}}'
status=$(curl -s -o /dev/null -w '%{http_code}' -H 'content-type: application/json' -d "$login" "$URL/api/v1/auth/login")
test "$status" = 200
echo "server smoke test passed"
