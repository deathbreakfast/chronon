#!/usr/bin/env bash
# BM-CH7 S4 prefill sweep: Q ∈ {1000,10000,100000} × postgres backends.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

export CARGO_BUILD_JOBS=1
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-target-chronon-bench}"
export CHRONON_BENCH_HARDWARE="${CHRONON_BENCH_HARDWARE:-aws-c6i-large}"
W="${CHRONON_BENCH_WORKER_COUNT:-32}"
REPORTS="profiling/chronon-bench/reports"

mkdir -p "$REPORTS"

for storage in postgres postgres-redis; do
  for q in 1000 10000 100000; do
    cargo run -p chronon-bench -- run \
      --experiment bm-ch7 \
      --storage "$storage" \
      --worker-count "$W" \
      --prefill "$q" \
      --hardware "$CHRONON_BENCH_HARDWARE" \
      --report "$REPORTS/bm-ch7-${storage}-q${q}-w${W}-${CHRONON_BENCH_HARDWARE}.json"
  done
done
