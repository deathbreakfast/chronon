#!/usr/bin/env bash
# T7: BM-CH7D Track B validation (execute tax).
set -euo pipefail

SF="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck disable=SC1091
source "$SF/lib/tier-common.sh"

export CHRONON_BENCH_TIER=T7
export CHRONON_DATA_TIER_PROFILE=ch7d-track-b

echo "=== T7 BM-CH7D $(date -u +%Y-%m-%dT%H:%M:%SZ) ===" | tee -a "$TIER_LOG"

tier_provision_bench_d3
tier_deploy_bench

# Prefer multicell cell0 when data-services has no Postgres (post-T5 fleet).
if [[ -f "$SF/instances.cells.env" ]]; then
  # shellcheck disable=SC1091
  source "$SF/instances.cells.env"
  export CHRONON_POSTGRES_URL="postgres://chronon:chronon@${CELL_0_POSTGRES_IP}:5432/chronon"
  export CHRONON_REDIS_URL="redis://${CELL_0_REDIS_IP}:6379"
  echo "T7 using cell0 ${CELL_0_POSTGRES_IP}" | tee -a "$TIER_LOG"
fi

for wn in 1 2 4 8 16; do
  tier_run_bench "t7-a-wn${wn}" "bm-ch7d-t7-wn${wn}-q100000-aws-c6i-large.json" \
    "\$BENCH run --experiment bm-ch7d --storage postgres-redis --worker-count ${wn} \
      --prefill 100000 --pool-count 1 --hardware \${CHRONON_BENCH_HARDWARE} \
      --report ~/chronon/profiling/chronon-bench/reports/bm-ch7d-t7-wn${wn}-q100000-aws-c6i-large.json"
done

host="$(tier_bench_host)"
# shellcheck disable=SC2207
SSH_OPTS=($(tier_ssh_opts))
ssh "${SSH_OPTS[@]}" "ec2-user@${host}" bash -s <<'EOF' | tee -a "$TIER_LOG"
set -euo pipefail
export PATH=$HOME/chronon-bench/bin:$PATH
chronon-bench scaling-curve ch7d-fleet-curve \
  --storage postgres-redis --hardware aws-c6i-large \
  --reports-dir ~/chronon/profiling/chronon-bench/reports
EOF

tier_finish T7
echo "T7 complete"
