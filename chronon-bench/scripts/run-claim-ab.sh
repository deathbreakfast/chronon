#!/usr/bin/env bash
# BM-CH7 postgres vs postgres-redis A/B at fixed W (default 32).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

export CARGO_BUILD_JOBS=1
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-target-chronon-bench}"
export CHRONON_BENCH_HARDWARE="${CHRONON_BENCH_HARDWARE:-aws-c6i-large}"
W="${CHRONON_BENCH_WORKER_COUNT:-32}"
PREFILL="${CHRONON_BENCH_PREFILL:-10000}"
REPORTS="profiling/chronon-bench/reports"

mkdir -p "$REPORTS"

for storage in postgres postgres-redis; do
  cargo run -p chronon-bench -- run \
    --experiment bm-ch7 \
    --storage "$storage" \
    --worker-count "$W" \
    --prefill "$PREFILL" \
    --hardware "$CHRONON_BENCH_HARDWARE" \
    --report "$REPORTS/bm-ch7-ab-${storage}-w${W}-${CHRONON_BENCH_HARDWARE}.json"
done

echo "Compare claim_ops_per_sec in:"
echo "  $REPORTS/bm-ch7-ab-postgres-w${W}-${CHRONON_BENCH_HARDWARE}.json"
echo "  $REPORTS/bm-ch7-ab-postgres-redis-w${W}-${CHRONON_BENCH_HARDWARE}.json"
