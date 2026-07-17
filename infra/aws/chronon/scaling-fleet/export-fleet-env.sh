#!/usr/bin/env bash
# Export CHRONON_* URLs for scaling fleet (source this file).
set -euo pipefail

SF="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck disable=SC1091
source "${SF}/instances.env" 2>/dev/null || source "${SF}/instances.env.example"

PG_PORT="${PGBOUNCER_PORT:-5432}"
echo "export CHRONON_POSTGRES_URL=postgres://chronon:chronon@${POSTGRES_IP}:${PG_PORT}/chronon"
echo "export CHRONON_REDIS_URL=redis://${REDIS_IP}:6379"
echo "export CHRONON_BENCH_STORAGE_TOPOLOGY=${STORAGE_TOPOLOGY:-postgres-redis-colocated}"
echo "export CHRONON_DATA_TIER_PROFILE=${DATA_TIER_PROFILE:-colocated-t3}"
echo "export CHRONON_BENCH_HARDWARE=aws-c6i-large"
echo "export BENCH_COUNT=${BENCH_COUNT:-1}"
if [[ -n "${CHRONON_REDIS_CLUSTER_URLS:-}" ]]; then
  echo "export CHRONON_REDIS_CLUSTER_URLS=${CHRONON_REDIS_CLUSTER_URLS}"
fi
if [[ -n "${CHRONON_REDIS_HASH_TAGS:-}" ]]; then
  echo "export CHRONON_REDIS_HASH_TAGS=${CHRONON_REDIS_HASH_TAGS}"
fi
