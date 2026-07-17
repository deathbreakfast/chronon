#!/usr/bin/env bash
# CH7-D0: worker sweep — default W ∈ {1,4,16,32}; override via CHRONON_D0_WORKERS.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

export CARGO_BUILD_JOBS=1
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-target-chronon-bench}"
export CHRONON_BENCH_HARDWARE="${CHRONON_BENCH_HARDWARE:-aws-c6i-large}"
PREFILL="${CHRONON_BENCH_PREFILL:-100000}"
REPORTS="profiling/chronon-bench/reports"
STORAGE="${CHRONON_BENCH_STORAGE:-postgres-redis}"
WORKERS="${CHRONON_D0_WORKERS:-1 4 16 32}"

mkdir -p "$REPORTS"

for w in $WORKERS; do
  cargo run -p chronon-bench -- run \
    --experiment bm-ch7 \
    --storage "$STORAGE" \
    --worker-count "$w" \
    --prefill "$PREFILL" \
    --hardware "$CHRONON_BENCH_HARDWARE" \
    --report "$REPORTS/bm-ch7-${STORAGE}-w${w}-${CHRONON_BENCH_HARDWARE}.json"
done

cargo run -p chronon-bench -- scaling-curve ch7-worker-curve \
  --storage "$STORAGE" \
  --hardware "$CHRONON_BENCH_HARDWARE" \
  --reports-dir "$REPORTS"
