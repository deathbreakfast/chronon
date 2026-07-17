#!/usr/bin/env bash
# T0: full 1 Redis + 1 Postgres ceiling matrix (colocated t3.medium).
set -euo pipefail

SF="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck disable=SC1091
source "$SF/lib/tier-common.sh"

export CHRONON_BENCH_TIER=T0
export CHRONON_DATA_TIER_PROFILE=colocated-t3
export CHRONON_DATA_INSTANCE_TYPE=t3.medium

echo "=== T0 single-node ceiling matrix $(date -u +%Y-%m-%dT%H:%M:%SZ) ===" | tee -a "$TIER_LOG"

"$SF/ensure-data-services.sh" 2>&1 | tee -a "$TIER_LOG"
tier_provision_bench_d3
"$SF/bootstrap-data.sh" 2>&1 | tee -a "$TIER_LOG"
if [[ "${CHRONON_SKIP_REMOTE_BUILD:-}" != "1" ]]; then
  tier_deploy_bench
fi

# T0-A: W sweep
for w in 1 2 4 8 16 32 64; do
  tier_ch7_run "t0-a-w${w}" "bm-ch7-t0-w${w}-k1-q100000-colocated-t3-aws-c6i-large.json" \
    "$w" 1 100000
done

# T0-B: K sweep @ W=1
for k in 1 2 4 8 16 32 64; do
  tier_ch7_run "t0-b-k${k}" "bm-ch7-t0-k${k}-w1-q100000-colocated-t3-aws-c6i-large.json" \
    1 "$k" 100000
done

# T0-C: K sweep @ W=16
for k in 1 4 16 64; do
  tier_ch7_run "t0-c-k${k}-w16" "bm-ch7-t0-k${k}-w16-q100000-colocated-t3-aws-c6i-large.json" \
    16 "$k" 100000
done

# T0-D: Q sweep
for q in 1000 10000 100000; do
  tier_ch7_run "t0-d-q${q}" "bm-ch7-t0-q${q}-w16-k1-colocated-t3-aws-c6i-large.json" \
    16 1 "$q"
done

# T0-E: postgres-only vs postgres-redis @ W=32
tier_run_bench "t0-e-pg-redis" "bm-ch7-t0-postgres-redis-w32-q100000-colocated-t3-aws-c6i-large.json" "" \
  "\$BENCH run --experiment bm-ch7 --storage postgres-redis --worker-count 32 --prefill 100000 \
    --pool-count 1 --hardware \${CHRONON_BENCH_HARDWARE} \
    --report ~/chronon/profiling/chronon-bench/reports/bm-ch7-t0-postgres-redis-w32-q100000-colocated-t3-aws-c6i-large.json"
tier_run_bench "t0-e-postgres" "bm-ch7-t0-postgres-w32-q100000-colocated-t3-aws-c6i-large.json" "" \
  "\$BENCH run --experiment bm-ch7 --storage postgres --worker-count 32 --prefill 100000 \
    --pool-count 1 --hardware \${CHRONON_BENCH_HARDWARE} \
    --report ~/chronon/profiling/chronon-bench/reports/bm-ch7-t0-postgres-w32-q100000-colocated-t3-aws-c6i-large.json"

# T0-G: PG pool size sweep @ W=16
for ps in 5 10 25 50; do
  export CHRONON_PG_POOL_SIZE="$ps"
  tier_ch7_run "t0-g-pool${ps}" "bm-ch7-t0-pgpool${ps}-w16-k1-q100000-colocated-t3-aws-c6i-large.json" \
    16 1 100000
done
unset CHRONON_PG_POOL_SIZE

# T0-F: multibench sanity (orchestrated from bench_0)
export CHRONON_BENCH_WORKERS_PER_HOST=1
export CHRONON_BENCH_PREFILL=100000
export CHRONON_CH7_MULTIBENCH_BC="1 2 4"
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
chmod +x ~/chronon/scaling-fleet/scripts/run-ch7-d3-fleet-local.sh
export CHRONON_SSH_KEY=$HOME/.ssh/chronon-bench.pem
export CHRONON_BENCH_TIER=T0-F
export CHRONON_DATA_TIER_PROFILE=colocated-t3
export CHRONON_BENCH_CELL_ID_PREFIX=bm-ch7-t0-bc
~/chronon/scaling-fleet/scripts/run-ch7-d3-fleet-local.sh
REMOTE

tier_eval_fleet_env
host="$(tier_bench_host)"
ssh "${SSH_OPTS[@]}" "ec2-user@${host}" bash -s <<EOF | tee -a "$TIER_LOG"
set -euo pipefail
export PATH=\$HOME/chronon-bench/bin:\$PATH
\$HOME/chronon-bench/bin/chronon-bench scaling-curve ch7-worker-curve \
  --storage postgres-redis --hardware \${CHRONON_BENCH_HARDWARE:-aws-c6i-large} \
  --reports-dir ~/chronon/profiling/chronon-bench/reports
\$HOME/chronon-bench/bin/chronon-bench scaling-curve ch7-pool-curve \
  --storage postgres-redis --hardware \${CHRONON_BENCH_HARDWARE:-aws-c6i-large} \
  --reports-dir ~/chronon/profiling/chronon-bench/reports
EOF

tier_finish T0
echo "T0 complete"
