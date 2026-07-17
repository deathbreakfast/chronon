#!/usr/bin/env bash
# BM-CH1 S3 partition sweep: P ∈ {4,16,64} × postgres backends.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

export CARGO_BUILD_JOBS=1
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-target-chronon-bench}"
export CHRONON_BENCH_HARDWARE="${CHRONON_BENCH_HARDWARE:-aws-t3-medium}"
REPORTS="profiling/chronon-bench/reports"

mkdir -p "$REPORTS"

for storage in mem sqlite postgres postgres-redis; do
  for p in 4 16 64; do
    cargo run -p chronon-bench -- run \
      --experiment bm-ch1 \
      --storage "$storage" \
      --partitions "$p" \
      --jobs 1000 \
      --ops 50 \
      --hardware "$CHRONON_BENCH_HARDWARE" \
      --report "$REPORTS/bm-ch1-${storage}-p${p}-${CHRONON_BENCH_HARDWARE}.json"
  done
done
