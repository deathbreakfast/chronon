#!/usr/bin/env bash
# BM-CHL0–3 sustain ladder S2 across a storage backend.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

STORAGE="${1:-mem}"

export CARGO_BUILD_JOBS=1
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-target-chronon-bench}"
export CHRONON_BENCH_HARDWARE="${CHRONON_BENCH_HARDWARE:-aws-t3-medium}"
REPORTS="profiling/chronon-bench/reports"

mkdir -p "$REPORTS"

cargo run -p chronon-bench -- matrix \
  --slice scheduler-sustain \
  --storage "$STORAGE" \
  --hardware "$CHRONON_BENCH_HARDWARE" \
  --reports-dir "$REPORTS"

cargo run -p chronon-bench -- scaling-curve chl-sustain-curve \
  --storage "$STORAGE" \
  --hardware "$CHRONON_BENCH_HARDWARE" \
  --reports-dir "$REPORTS"
