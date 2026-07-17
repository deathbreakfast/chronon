#!/usr/bin/env bash
# Bootstrap Redis on redis host and Postgres on postgres host.
set -euo pipefail

SF="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck disable=SC1091
source "$SF/instances.env"
source "$SF/lib/bench-fleet.sh"
SSH_KEY="$(bench_ssh_key)"
SSH_OPTS=(-o StrictHostKeyChecking=no -o ConnectTimeout=30 -i "$SSH_KEY")

REDIS_HOST="ec2-user@${REDIS_PUBLIC_IP:-${REDIS_IP}}"
PG_HOST="ec2-user@${POSTGRES_PUBLIC_IP:-${POSTGRES_IP}}"

ssh "${SSH_OPTS[@]}" "$REDIS_HOST" bash -s <<'REMOTE'
set -euo pipefail
if ! command -v docker >/dev/null 2>&1; then
  sudo dnf install -y docker
  sudo systemctl enable --now docker
  sudo usermod -aG docker ec2-user
fi
docker rm -f chronon-redis 2>/dev/null || true
sudo docker run -d --name chronon-redis --restart unless-stopped \
  -p 6379:6379 redis:7-alpine
for i in $(seq 1 30); do
  sudo docker exec chronon-redis redis-cli ping | grep -q PONG && break
  sleep 2
done
REMOTE

ssh "${SSH_OPTS[@]}" "$PG_HOST" bash -s <<'REMOTE'
set -euo pipefail
if ! command -v docker >/dev/null 2>&1; then
  sudo dnf install -y docker
  sudo systemctl enable --now docker
  sudo usermod -aG docker ec2-user
fi
docker rm -f chronon-postgres 2>/dev/null || true
sudo docker run -d --name chronon-postgres --restart unless-stopped \
  -e POSTGRES_USER=chronon -e POSTGRES_PASSWORD=chronon -e POSTGRES_DB=chronon \
  -p 5432:5432 postgres:16-alpine
for i in $(seq 1 30); do
  sudo docker exec chronon-postgres pg_isready -U chronon -d chronon >/dev/null 2>&1 && break
  sleep 2
done
REMOTE

echo "split bootstrap complete"
