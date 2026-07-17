#!/usr/bin/env bash
# T2: sized single pair (r6g.xlarge colocated or split from env).
set -euo pipefail

SF="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck disable=SC1091
source "$SF/lib/tier-common.sh"

export CHRONON_BENCH_TIER=T2
export CHRONON_REDIS_INSTANCE_TYPE="${CHRONON_REDIS_INSTANCE_TYPE:-c6i.xlarge}"
export CHRONON_POSTGRES_INSTANCE_TYPE="${CHRONON_POSTGRES_INSTANCE_TYPE:-c6i.xlarge}"
export CHRONON_DATA_TIER_PROFILE=sized-c6i-xlarge

echo "=== T2 sized nodes $(date -u +%Y-%m-%dT%H:%M:%SZ) ===" | tee -a "$TIER_LOG"

if [[ "${CHRONON_T2_TOPOLOGY:-split}" == "colocated" ]]; then
  # Reuse colocated bootstrap on a new sized host via ensure-data-services resize path.
  export CHRONON_DATA_INSTANCE_TYPE=r6g.xlarge
  "$SF/ensure-data-services.sh" 2>&1 | tee -a "$TIER_LOG"
  "$SF/bootstrap-data.sh" 2>&1 | tee -a "$TIER_LOG"
else
  "$SF/provision-data-tier-split.sh" 2>&1 | tee -a "$TIER_LOG"
fi

tier_provision_bench_d3
tier_deploy_bench

for w in 1 4 8 16 32 64; do
  tier_ch7_run "t2-a-w${w}" "bm-ch7-t2-w${w}-k1-q100000-sized-r6g-xlarge-aws-c6i-large.json" \
    "$w" 1 100000
done

for k in 1 16 64; do
  tier_ch7_run "t2-b-k${k}-w1" "bm-ch7-t2-k${k}-w1-q100000-sized-r6g-xlarge-aws-c6i-large.json" \
    1 "$k" 100000
  tier_ch7_run "t2-b-k${k}-w16" "bm-ch7-t2-k${k}-w16-q100000-sized-r6g-xlarge-aws-c6i-large.json" \
    16 "$k" 100000
done

tier_finish T2
echo "T2 complete"
