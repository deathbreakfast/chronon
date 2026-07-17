#!/usr/bin/env bash
# BM-CH1 job sweep S1: J ∈ {1000,10000,100000} × all backends.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

export CARGO_BUILD_JOBS=1
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-target-chronon-bench}"
export CHRONON_BENCH_HARDWARE="${CHRONON_BENCH_HARDWARE:-aws-t3-medium}"
REPORTS="profiling/chronon-bench/reports"

mkdir -p "$REPORTS"

for storage in mem sqlite postgres postgres-redis; do
  for j in 1000 10000 100000; do
    cargo run -p chronon-bench -- run \
      --experiment bm-ch1 \
      --storage "$storage" \
      --jobs "$j" \
      --ops 50 \
      --hardware "$CHRONON_BENCH_HARDWARE" \
      --report "$REPORTS/bm-ch1-${storage}-j${j}-${CHRONON_BENCH_HARDWARE}.json"
  done
  cargo run -p chronon-bench -- scaling-curve ch1-job-curve \
    --storage "$storage" \
    --hardware "$CHRONON_BENCH_HARDWARE" \
    --reports-dir "$REPORTS"
done
