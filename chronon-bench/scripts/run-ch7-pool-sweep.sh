#!/usr/bin/env bash
# CH7-D1: pool sweep K ∈ {1,4,16}; W from CHRONON_BENCH_WORKER_COUNT (default 16).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

export CARGO_BUILD_JOBS=1
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-target-chronon-bench}"
export CHRONON_BENCH_HARDWARE="${CHRONON_BENCH_HARDWARE:-aws-c6i-large}"
PREFILL="${CHRONON_BENCH_PREFILL:-100000}"
W="${CHRONON_BENCH_WORKER_COUNT:-16}"
POOLS="${CHRONON_D1_POOLS:-1 4 16}"
REPORTS="profiling/chronon-bench/reports"
STORAGE="${CHRONON_BENCH_STORAGE:-postgres-redis}"

mkdir -p "$REPORTS"

for k in $POOLS; do
  cargo run -p chronon-bench -- run \
    --experiment bm-ch7 \
    --storage "$STORAGE" \
    --worker-count "$W" \
    --prefill "$PREFILL" \
    --pool-count "$k" \
    --pool-layout distinct \
    --hardware "$CHRONON_BENCH_HARDWARE" \
    --report "$REPORTS/bm-ch7-${STORAGE}-k${k}-w${W}-${CHRONON_BENCH_HARDWARE}.json"
done

cargo run -p chronon-bench -- scaling-curve ch7-pool-curve \
  --storage "$STORAGE" \
  --hardware "$CHRONON_BENCH_HARDWARE" \
  --reports-dir "$REPORTS"
