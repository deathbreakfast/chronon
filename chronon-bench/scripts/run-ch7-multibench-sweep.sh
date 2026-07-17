#!/usr/bin/env bash
# CH7-D3: multibench bc ∈ {1,2,4} with Q=100k default.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

export CARGO_BUILD_JOBS=1
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-target-chronon-bench}"
export CHRONON_BENCH_HARDWARE="${CHRONON_BENCH_HARDWARE:-aws-c6i-large}"
PREFILL="${CHRONON_BENCH_PREFILL:-100000}"
W="${CHRONON_BENCH_WORKER_COUNT:-16}"
REPORTS="profiling/chronon-bench/reports"
STORAGE="${CHRONON_BENCH_STORAGE:-postgres-redis}"
BC_VALUES="${CHRONON_CH7_MULTIBENCH_BC:-1 2 4}"

mkdir -p "$REPORTS"

run_cell() {
  local bc="$1"
  local w_per_host=$(( W / bc ))
  [[ "$w_per_host" -ge 1 ]] || w_per_host=1
  local tag="bm-ch7-bc${bc}-w${w_per_host}-q${PREFILL}"
  for i in $(seq 0 $((bc - 1))); do
    local drain_env=()
    if [[ "$i" -gt 0 ]]; then
      drain_env=(CHRONON_BENCH_DRAIN_ONLY=1)
    fi
    env "${drain_env[@]}" \
      CHRONON_BENCH_CLIENT_INDEX="$i" \
      CHRONON_BENCH_CLIENT_COUNT="$bc" \
      cargo run -p chronon-bench -- run \
        --experiment bm-ch7 \
        --storage "$STORAGE" \
        --worker-count "$w_per_host" \
        --prefill "$PREFILL" \
        --bench-client-index "$i" \
        --bench-client-count "$bc" \
        --hardware "$CHRONON_BENCH_HARDWARE" \
        --report "$REPORTS/${tag}-i${i}-${STORAGE}-${CHRONON_BENCH_HARDWARE}.json" &
  done
  wait
  if [[ "$bc" -gt 1 ]]; then
    cargo run -p chronon-bench -- aggregate \
      --storage "$STORAGE" \
      --hardware "$CHRONON_BENCH_HARDWARE" \
      --reports-dir "$REPORTS" \
      --cell-prefix "$tag"
  fi
}

for bc in $BC_VALUES; do
  run_cell "$bc"
done

cargo run -p chronon-bench -- scaling-curve ch7-multibench-curve \
  --storage "$STORAGE" \
  --hardware "$CHRONON_BENCH_HARDWARE" \
  --reports-dir "$REPORTS"
