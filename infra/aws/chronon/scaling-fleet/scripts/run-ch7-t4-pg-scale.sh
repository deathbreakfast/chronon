#!/usr/bin/env bash
# T4: Postgres scaling — PgBouncer + pool size sweep.
set -euo pipefail

SF="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck disable=SC1091
source "$SF/lib/tier-common.sh"

export CHRONON_BENCH_TIER=T4
export CHRONON_DATA_TIER_PROFILE=pg-scaled

echo "=== T4 PG scale $(date -u +%Y-%m-%dT%H:%M:%SZ) ===" | tee -a "$TIER_LOG"

"$SF/bootstrap-pgbouncer.sh" 2>&1 | tee -a "$TIER_LOG"
tier_provision_bench_d3
tier_deploy_bench

for ps in 5 10 25 50 100; do
  export CHRONON_PG_POOL_SIZE="$ps"
  tier_ch7_run "t4-a-pool${ps}" "bm-ch7-t4-pgpool${ps}-w16-k1-q100000-pgbouncer-aws-c6i-large.json" \
    16 1 100000
done
unset CHRONON_PG_POOL_SIZE

tier_finish T4
echo "T4 complete"
