#!/usr/bin/env bash
# Embedded SQLite midnight-burst campaign: W=4/8/16/32 on a single host.
# Hardware: aws-t3.medium. Storage: sqlite. Deployment: embedded.
# Workload: 500 due jobs (80/15/5 mix of 100/250/500 ms sleep) plus 60
# recurring jobs (hourly / 15-minute / 5-minute) that collide with the burst.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-1}"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-target-chronon-bench}"
export CHRONON_BENCH_HARDWARE="${CHRONON_BENCH_HARDWARE:-aws-t3-medium}"
export CHRONON_TICK_BATCH_LIMIT="${CHRONON_TICK_BATCH_LIMIT:-500}"

JOBS="${CHRONON_BENCH_BURST_JOBS:-500}"
STORAGE="${CHRONON_BENCH_STORAGE:-sqlite}"

for workers in 4 8 16 32; do
  echo "bm-ch-embed-burst W=${workers} jobs=${JOBS} storage=${STORAGE}"
  CHRONON_WORKER_CONCURRENCY="${workers}" \
  CHRONON_EXECUTOR_CONCURRENCY="${workers}" \
  cargo run -p chronon-bench --release -- run \
    --experiment bm-ch-embed-burst \
    --storage "${STORAGE}" \
    --deployment embedded \
    --telemetry off \
    --topology isolated-lab \
    --jobs "${JOBS}" \
    --worker-count "${workers}" \
    --hardware "${CHRONON_BENCH_HARDWARE}"
done
