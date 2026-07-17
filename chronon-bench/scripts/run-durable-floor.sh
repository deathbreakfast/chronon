#!/usr/bin/env bash
# Durable scheduler floor: BM-CH0 + BM-CH1 across sqlite, postgres, postgres-redis.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

export CARGO_BUILD_JOBS=1
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-target-chronon-bench}"
export CHRONON_BENCH_HARDWARE="${CHRONON_BENCH_HARDWARE:-aws-t3-medium}"

for storage in sqlite postgres postgres-redis; do
  cargo run -p chronon-bench -- matrix \
    --slice durable-floor \
    --storage "$storage" \
    --hardware "$CHRONON_BENCH_HARDWARE"
done
