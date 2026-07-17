#!/usr/bin/env bash
# BM-CH7 worker sweep S0: W ∈ {8,16,32,64} × postgres, postgres-redis.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

export CARGO_BUILD_JOBS=1
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-target-chronon-bench}"
export CHRONON_BENCH_HARDWARE="${CHRONON_BENCH_HARDWARE:-aws-c6i-large}"
PREFILL="${CHRONON_BENCH_PREFILL:-10000}"
REPORTS="profiling/chronon-bench/reports"

mkdir -p "$REPORTS"

for storage in postgres postgres-redis; do
  for w in 8 16 32 64; do
    cargo run -p chronon-bench -- run \
      --experiment bm-ch7 \
      --storage "$storage" \
      --worker-count "$w" \
      --prefill "$PREFILL" \
      --hardware "$CHRONON_BENCH_HARDWARE" \
      --report "$REPORTS/bm-ch7-${storage}-w${w}-${CHRONON_BENCH_HARDWARE}.json"
  done
  cargo run -p chronon-bench -- scaling-curve ch7-worker-curve \
    --storage "$storage" \
    --hardware "$CHRONON_BENCH_HARDWARE" \
    --reports-dir "$REPORTS"
done
