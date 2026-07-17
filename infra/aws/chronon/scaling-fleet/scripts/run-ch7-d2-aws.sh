#!/usr/bin/env bash
# CH7-D2: colocated vs split data topology A/B (single bench).
set -euo pipefail

SF="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck disable=SC1091
source "$SF/lib/bench-fleet.sh"
eval "$("$SF/export-fleet-env.sh")"

SSH_KEY="${CHRONON_SSH_KEY:?set CHRONON_SSH_KEY to the SSH private key path}"
SSH_OPTS=(-o StrictHostKeyChecking=no -o ConnectTimeout=30 -i "$SSH_KEY")
HOST="$(resolve_bench_ip 1)"
REPORTS="profiling/chronon-bench/reports"
W="${CHRONON_BENCH_WORKER_COUNT:-16}"
PREFILL="${CHRONON_BENCH_PREFILL:-10000}"
STORAGE="${CHRONON_BENCH_STORAGE:-postgres-redis}"

for topo in postgres-redis-colocated postgres-redis-split; do
  ssh "${SSH_OPTS[@]}" "ec2-user@${HOST}" bash -s <<EOF
set -euo pipefail
export CHRONON_POSTGRES_URL=${CHRONON_POSTGRES_URL}
export CHRONON_REDIS_URL=${CHRONON_REDIS_URL}
export CHRONON_BENCH_HARDWARE=${CHRONON_BENCH_HARDWARE}
export CARGO_BUILD_JOBS=1
export CARGO_TARGET_DIR=target-chronon-bench
source ~/.cargo/env 2>/dev/null || true
cd ~/chronon
cargo run -p chronon-bench -- run \
  --experiment bm-ch7 \
  --storage ${STORAGE} \
  --worker-count ${W} \
  --prefill ${PREFILL} \
  --storage-topology ${topo} \
  --hardware ${CHRONON_BENCH_HARDWARE} \
  --report ${REPORTS}/bm-ch7-${STORAGE}-d2-${topo}-${CHRONON_BENCH_HARDWARE}.json
EOF
done

ssh "${SSH_OPTS[@]}" "ec2-user@${HOST}" bash -s <<EOF
set -euo pipefail
source ~/.cargo/env 2>/dev/null || true
cd ~/chronon
cargo run -p chronon-bench -- scaling-curve ch7-data-curve \
  --storage ${STORAGE} \
  --hardware ${CHRONON_BENCH_HARDWARE} \
  --reports-dir ${REPORTS}
EOF

"$SF/scripts/fetch-reports.sh"
