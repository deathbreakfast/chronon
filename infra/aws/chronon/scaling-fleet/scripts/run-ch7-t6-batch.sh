#!/usr/bin/env bash
# T6: batched sync claim sweep + re-run T5 subset.
set -euo pipefail

SF="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck disable=SC1091
source "$SF/lib/tier-common.sh"

export CHRONON_BENCH_TIER=T6
export CHRONON_DATA_TIER_PROFILE=claim-batch

echo "=== T6 claim batch $(date -u +%Y-%m-%dT%H:%M:%SZ) ===" | tee -a "$TIER_LOG"

tier_provision_bench_d3
tier_deploy_bench

# data-services may only have Redis Cluster left after T3/T5; prefer cell0 for T6-A.
if [[ -f "$SF/instances.cells.env" ]]; then
  # shellcheck disable=SC1091
  source "$SF/instances.cells.env"
  export CHRONON_POSTGRES_URL="postgres://chronon:chronon@${CELL_0_POSTGRES_IP}:5432/chronon"
  export CHRONON_REDIS_URL="redis://${CELL_0_REDIS_IP}:6379"
  echo "T6-A using cell0 ${CELL_0_POSTGRES_IP}" | tee -a "$TIER_LOG"
fi

# T6-A: batch size sweep on best single cell (reuse current data tier).
for batch in 1 4 8 16; do
  export CHRONON_CLAIM_BATCH="$batch"
  tier_ch7_run "t6-a-batch${batch}" "bm-ch7-t6-batch${batch}-w16-k1-q100000-aws-c6i-large.json" \
    16 1 100000
done
unset CHRONON_CLAIM_BATCH

# T6-B: re-run T5 @ N=4,8 with optimal batch (16).
export CHRONON_CLAIM_BATCH=16
export CHRONON_T5_CELL_LADDER="4 8"
if [[ -f "$SF/instances.cells.env" ]]; then
  BENCH0="$(tier_bench_host)"
  # shellcheck disable=SC2207
  SSH_OPTS=($(tier_ssh_opts))
  ssh "${SSH_OPTS[@]}" "ec2-user@${BENCH0}" \
    'mkdir -p ~/chronon/scaling-fleet/scripts ~/chronon/scaling-fleet/lib'
  scp "${SSH_OPTS[@]}" "$SF/instances.cells.env" "ec2-user@${BENCH0}:~/chronon/scaling-fleet/instances.cells.env"
  scp "${SSH_OPTS[@]}" "$SF/lib/"*.sh "ec2-user@${BENCH0}:~/chronon/scaling-fleet/lib/" 2>/dev/null || true
  scp "${SSH_OPTS[@]}" "$SF/scripts/run-ch7-t5-fleet-local.sh" "ec2-user@${BENCH0}:~/chronon/scaling-fleet/scripts/run-ch7-t5-fleet-local.sh"
  ssh "${SSH_OPTS[@]}" "ec2-user@${BENCH0}" bash -s <<'REMOTE' | tee -a "$TIER_LOG"
set -euo pipefail
export CHRONON_CLAIM_BATCH=16
export CHRONON_BENCH_TIER=T6-B
export CHRONON_T5_CELL_LADDER="4 8"
~/chronon/scaling-fleet/scripts/run-ch7-t5-fleet-local.sh
REMOTE
fi
unset CHRONON_CLAIM_BATCH

tier_finish T6
echo "T6 complete"
