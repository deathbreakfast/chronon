#!/usr/bin/env bash
# T3: Redis Cluster ceiling matrix.
set -euo pipefail

SF="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck disable=SC1091
source "$SF/lib/tier-common.sh"

export CHRONON_BENCH_TIER=T3
export CHRONON_DATA_TIER_PROFILE=redis-cluster

echo "=== T3 Redis Cluster $(date -u +%Y-%m-%dT%H:%M:%SZ) ===" | tee -a "$TIER_LOG"

"$SF/bootstrap-redis-cluster.sh" 2>&1 | tee -a "$TIER_LOG"
# shellcheck disable=SC1091
source "$SF/instances.env"
tier_eval_fleet_env
export CHRONON_REDIS_HASH_TAGS=1
export CHRONON_REDIS_CLUSTER_URLS

tier_provision_bench_d3
tier_deploy_bench

# T3-B: K sweep @ W=1
for k in 1 4 16 32 64; do
  tier_ch7_run "t3-b-k${k}-w1" "bm-ch7-t3-k${k}-w1-q100000-cluster-aws-c6i-large.json" \
    1 "$k" 100000
done

# T3-C: K sweep @ W=16
for k in 16 32 64; do
  tier_ch7_run "t3-c-k${k}-w16" "bm-ch7-t3-k${k}-w16-q100000-cluster-aws-c6i-large.json" \
    16 "$k" 100000
done

# T3-D: multibench bc=4
export CHRONON_BENCH_WORKERS_PER_HOST=1
export CHRONON_BENCH_PREFILL=100000
export CHRONON_CH7_MULTIBENCH_BC="4"
export CHRONON_BENCH_POOL_LAYOUT=distinct
BENCH0="$(tier_bench_host)"
# shellcheck disable=SC2207
SSH_OPTS=($(tier_ssh_opts))
scp "${SSH_OPTS[@]}" "$SF/instances.env" "ec2-user@${BENCH0}:~/chronon/scaling-fleet/instances.env"
scp "${SSH_OPTS[@]}" "$SF/scripts/run-ch7-d3-fleet-local.sh" "ec2-user@${BENCH0}:~/chronon/scaling-fleet/scripts/run-ch7-d3-fleet-local.sh"
scp "${SSH_OPTS[@]}" "$(bench_ssh_key)" "ec2-user@${BENCH0}:~/.ssh/chronon-bench.pem"
ssh "${SSH_OPTS[@]}" "ec2-user@${BENCH0}" bash -s <<'REMOTE' | tee -a "$TIER_LOG"
set -euo pipefail
chmod 600 ~/.ssh/chronon-bench.pem
export CHRONON_SSH_KEY=$HOME/.ssh/chronon-bench.pem
export CHRONON_BENCH_TIER=T3-D
export CHRONON_DATA_TIER_PROFILE=redis-cluster
export CHRONON_REDIS_HASH_TAGS=1
export CHRONON_REDIS_CLUSTER_URLS="$(grep CHRONON_REDIS_CLUSTER_URLS ~/chronon/scaling-fleet/instances.env | cut -d= -f2-)"
export CHRONON_BENCH_CELL_ID_PREFIX=bm-ch7-t3-bc
~/chronon/scaling-fleet/scripts/run-ch7-d3-fleet-local.sh
REMOTE

tier_finish T3
echo "T3 complete"
