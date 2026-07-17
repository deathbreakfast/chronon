#!/usr/bin/env bash
# CH7-D4: worker daemon fleet Wn ∈ {1,2,4} (Track B).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

export CARGO_BUILD_JOBS=1
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-target-chronon-bench}"
export CHRONON_BENCH_HARDWARE="${CHRONON_BENCH_HARDWARE:-aws-c6i-large}"
export CHRONON_WORKER_CONCURRENCY="${CHRONON_WORKER_CONCURRENCY:-1}"
PREFILL="${CHRONON_BENCH_PREFILL:-100000}"
REPORTS="profiling/chronon-bench/reports"
STORAGE="${CHRONON_BENCH_STORAGE:-postgres-redis}"

mkdir -p "$REPORTS"

for wn in 1 2 4; do
  cargo run -p chronon-bench -- run \
    --experiment bm-ch7d \
    --storage "$STORAGE" \
    --worker-hosts "$wn" \
    --prefill "$PREFILL" \
    --hardware "$CHRONON_BENCH_HARDWARE" \
    --report "$REPORTS/bm-ch7d-${STORAGE}-wn${wn}-${CHRONON_BENCH_HARDWARE}.json"
done

cargo run -p chronon-bench -- scaling-curve ch7d-fleet-curve \
  --storage "$STORAGE" \
  --hardware "$CHRONON_BENCH_HARDWARE" \
  --reports-dir "$REPORTS"
