#!/usr/bin/env bash
# Local multibench smoke: bc=2 on mem, aggregate + curve.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

export CHRONON_BENCH_HARDWARE=local
REPORTS="${ROOT}/profiling/chronon-bench/reports/multibench-smoke"
mkdir -p "$REPORTS"
TAG="bm-ch7-bc2-smoke"

prefill_and_drain() {
  local idx="$1"
  local drain_only="$2"
  local report="${REPORTS}/${TAG}-i${idx}-mem-local.json"
  export CHRONON_BENCH_CLIENT_INDEX="$idx"
  export CHRONON_BENCH_CLIENT_COUNT=2
  if [[ "$drain_only" == "1" ]]; then
    export CHRONON_BENCH_DRAIN_ONLY=1
  else
    unset CHRONON_BENCH_DRAIN_ONLY
  fi
  cargo run -p chronon-bench -- run \
    --experiment bm-ch7 \
    --storage mem \
    --worker-count 4 \
    --prefill 200 \
    --pool-count 2 \
    --pool-layout distinct \
    --bench-client-index "$idx" \
    --bench-client-count 2 \
    --hardware local \
    --report "$report"
}

prefill_and_drain 0 0 &
pid0=$!
sleep 1
prefill_and_drain 1 1 &
pid1=$!
wait "$pid0"
wait "$pid1"

cargo run -p chronon-bench -- aggregate \
  --storage mem \
  --hardware local \
  --reports-dir "$REPORTS" \
  --cell-prefix "$TAG"

cargo run -p chronon-bench -- scaling-curve ch7-multibench-curve \
  --storage mem \
  --hardware local \
  --reports-dir "$REPORTS"

echo "multibench smoke OK — reports in $REPORTS"
