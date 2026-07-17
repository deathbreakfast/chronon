#!/usr/bin/env bash
# Bootstrap data tier on data-services host (colocated or split via env).
set -euo pipefail

SF="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CHRONON_AWS="$(cd "$SF/.." && pwd)"
# shellcheck disable=SC1091
source "$SF/lib/bench-fleet.sh"

SSH_KEY="$(bench_ssh_key)"
SSH_OPTS=(-o StrictHostKeyChecking=no -o ConnectTimeout=30 -i "$SSH_KEY")
DATA_HOST="ec2-user@$(resolve_data_ssh_host)"

echo "checking data tier on ${DATA_HOST}..."
if ! ssh "${SSH_OPTS[@]}" -o ConnectTimeout=10 "$DATA_HOST" 'true' 2>/dev/null; then
  echo "WARN: cannot SSH to data-services (likely old key pair); assuming Postgres+Redis still running since prior campaign"
  bench0="$(resolve_bench_ip 1)"
  ssh "${SSH_OPTS[@]}" "ec2-user@${bench0}" bash -s <<REMOTE
set -euo pipefail
PG=\${CHRONON_POSTGRES_URL:-postgres://chronon:chronon@${DATA_IP}:5432/chronon}
RD=\${CHRONON_REDIS_URL:-redis://${DATA_IP}:6379}
command -v psql >/dev/null 2>&1 || sudo dnf install -y postgresql15
psql "\$PG" -c 'select 1' >/dev/null
command -v redis-cli >/dev/null 2>&1 || sudo dnf install -y redis6
redis-cli -u "\$RD" ping | grep -q PONG
echo "data tier reachable from bench_0 via private IP ${DATA_IP}"
REMOTE
  exit 0
fi

# Only bootstrap if postgres/redis not running
if ssh "${SSH_OPTS[@]}" "$DATA_HOST" 'docker ps --format "{{.Names}}" | grep -q postgres'; then
  echo "data tier already running on ${DATA_HOST}"
  exit 0
fi

rsync -az -e "ssh ${SSH_OPTS[*]}" \
  "$CHRONON_AWS/docker-compose.data.yml" \
  "${DATA_HOST}:~/docker-compose.data.yml"

ssh "${SSH_OPTS[@]}" "$DATA_HOST" bash -s <<'REMOTE'
set -euo pipefail
if docker ps --format '{{.Names}}' | grep -q chronon-postgres; then
  exit 0
fi
docker rm -f chronon-postgres chronon-redis 2>/dev/null || true
docker run -d --name chronon-postgres --restart unless-stopped \
  -e POSTGRES_USER=chronon -e POSTGRES_PASSWORD=chronon -e POSTGRES_DB=chronon \
  -p 5432:5432 postgres:16-alpine
docker run -d --name chronon-redis --restart unless-stopped \
  -p 6379:6379 redis:7-alpine
for i in $(seq 1 30); do
  docker exec chronon-postgres pg_isready -U chronon -d chronon >/dev/null 2>&1 && break
  sleep 2
done
docker exec chronon-redis redis-cli ping | grep -q PONG
REMOTE
echo "data tier ready"
