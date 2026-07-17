#!/usr/bin/env bash
# T1: real split Redis + Postgres hosts.
set -euo pipefail

SF="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck disable=SC1091
source "$SF/lib/tier-common.sh"

export CHRONON_BENCH_TIER=T1
export CHRONON_DATA_TIER_PROFILE=split-c6i
export CHRONON_REDIS_INSTANCE_TYPE="${CHRONON_REDIS_INSTANCE_TYPE:-c6i.large}"
export CHRONON_POSTGRES_INSTANCE_TYPE="${CHRONON_POSTGRES_INSTANCE_TYPE:-c6i.large}"

echo "=== T1 split topology $(date -u +%Y-%m-%dT%H:%M:%SZ) ===" | tee -a "$TIER_LOG"

"$SF/provision-data-tier-split.sh" 2>&1 | tee -a "$TIER_LOG"
tier_provision_bench_d3
if [[ "${CHRONON_SKIP_REMOTE_BUILD:-}" != "1" ]]; then
  tier_deploy_bench
fi

for w in 1 4 16 32 64; do
  tier_ch7_run "t1-b-w${w}" "bm-ch7-t1-w${w}-k1-q100000-split-c6i-aws-c6i-large.json" \
    "$w" 1 100000
done

for k in 1 4 16 64; do
  tier_ch7_run "t1-c-k${k}-w1" "bm-ch7-t1-k${k}-w1-q100000-split-c6i-aws-c6i-large.json" \
    1 "$k" 100000
  tier_ch7_run "t1-c-k${k}-w16" "bm-ch7-t1-k${k}-w16-q100000-split-c6i-aws-c6i-large.json" \
    16 "$k" 100000
done

tier_ch7_run "t1-a-primary" "bm-ch7-t1-w16-k1-q100000-split-c6i-aws-c6i-large.json" \
  16 1 100000

tier_finish T1
echo "T1 complete"
