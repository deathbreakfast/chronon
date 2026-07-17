#!/usr/bin/env bash
# Full benchmark campaign orchestrator (run on AWS bench fleet after E2E gate).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

export CARGO_BUILD_JOBS=1
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-target-chronon-bench}"

echo "== durable floor =="
"$ROOT/chronon-bench/scripts/run-durable-floor.sh"

echo "== CH1 job sweep =="
"$ROOT/chronon-bench/scripts/run-ch1-job-sweep.sh"

echo "== CHL sustain =="
for storage in mem sqlite postgres postgres-redis; do
  "$ROOT/chronon-bench/scripts/run-chl-sustain.sh" "$storage"
done

echo "== execution path =="
"$ROOT/chronon-bench/scripts/run-execution-path.sh"

echo "== resilience =="
"$ROOT/chronon-bench/scripts/run-resilience.sh"

echo "== partition sweep =="
"$ROOT/chronon-bench/scripts/run-partition-sweep.sh"

echo "== CH7 worker sweep =="
CHRONON_BENCH_HARDWARE=aws-c6i-large "$ROOT/chronon-bench/scripts/run-ch7-worker-sweep.sh"

echo "== CH7 prefill sweep =="
"$ROOT/chronon-bench/scripts/run-prefill-sweep.sh"

echo "== claim A/B =="
"$ROOT/chronon-bench/scripts/run-claim-ab.sh"

echo "Campaign complete."
