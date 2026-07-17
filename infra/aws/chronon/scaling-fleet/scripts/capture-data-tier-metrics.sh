#!/usr/bin/env bash
# Sample Redis/Postgres/CPU on the data-services host during a bench run.
set -euo pipefail

SF="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck disable=SC1091
source "${SF}/instances.env"

LABEL="${1:-sample}"
OUT="${2:-$HOME/chronon-hyperscale-metrics.log}"
DURATION="${CHRONON_METRICS_DURATION_SEC:-60}"
INTERVAL="${CHRONON_METRICS_INTERVAL_SEC:-5}"

DATA_HOST="${DATA_HOST:-ec2-user@${DATA_PUBLIC_IP}}"
SSH_KEY="${CHRONON_SSH_KEY:?set CHRONON_SSH_KEY to the SSH private key path}"
SSH_OPTS=(-o StrictHostKeyChecking=no -o ConnectTimeout=15 -i "$SSH_KEY")

{
  echo "=== metrics ${LABEL} $(date -u +%Y-%m-%dT%H:%M:%SZ) duration=${DURATION}s ==="
  end=$(( $(date +%s) + DURATION ))
  while [[ $(date +%s) -lt $end ]]; do
    echo "--- $(date -u +%Y-%m-%dT%H:%M:%SZ) ---"
    ssh "${SSH_OPTS[@]}" "$DATA_HOST" bash -s <<'REMOTE' || true
set -euo pipefail
echo "cpu:"; top -bn1 | head -3
if command -v docker >/dev/null 2>&1; then
  cid=$(docker ps -q --filter name=redis | head -1)
  if [[ -n "$cid" ]]; then
    echo "redis:"; docker exec "$cid" redis-cli INFO stats 2>/dev/null | grep -E "instantaneous_ops_per_sec|total_commands_processed" || true
  fi
  pg=$(docker ps -q --filter name=postgres | head -1)
  if [[ -n "$pg" ]]; then
    echo "postgres:"; docker exec "$pg" psql -U chronon -d chronon -tAc \
      "SELECT numbackends, xact_commit, xact_rollback, blks_read, blks_hit FROM pg_stat_database WHERE datname='chronon';" 2>/dev/null || true
  fi
fi
REMOTE
    sleep "$INTERVAL"
  done
} >> "$OUT"

echo "appended metrics to $OUT"
