#!/usr/bin/env bash
# Resume benchmark campaign from resilience (after BM-CH4 fix).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

export CARGO_BUILD_JOBS=1
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-target-chronon-bench}"

echo "== resilience =="
"$ROOT/chronon-bench/scripts/run-resilience.sh"

echo "== partition sweep =="
"$ROOT/chronon-bench/scripts/run-partition-sweep.sh"

echo "== CH7 worker sweep =="
CHRONON_BENCH_HARDWARE=aws-t3-medium "$ROOT/chronon-bench/scripts/run-ch7-worker-sweep.sh"

echo "== CH7 prefill sweep =="
CHRONON_BENCH_HARDWARE=aws-t3-medium "$ROOT/chronon-bench/scripts/run-prefill-sweep.sh"

echo "== claim A/B =="
CHRONON_BENCH_HARDWARE=aws-t3-medium "$ROOT/chronon-bench/scripts/run-claim-ab.sh"

echo "Campaign resume: PASS"
