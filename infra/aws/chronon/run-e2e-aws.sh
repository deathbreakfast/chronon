#!/usr/bin/env bash
# Run full AWS E2E gate on e2e-runner (durable PR CI slice + distributed smokes).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$ROOT"

export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-1}"
export RUST_BACKTRACE="${RUST_BACKTRACE:-1}"

# Unit tests in chronon-backend-redis read CHRONON_TEST_REDIS_URL via RedisQueueLayer::test_url().
export CHRONON_TEST_REDIS_URL="${CHRONON_TEST_REDIS_URL:-${CHRONON_REDIS_URL:-}}"

echo "== Step 1: PR slice (mem + sqlite) =="
cargo test -p chronon-e2e -p chronon-axum -- --test-threads=1

echo "== Step 2: Store contracts + leader =="
cargo test -p chronon-backend-postgres --tests -- --include-ignored
cargo test -p chronon-backend-redis --tests -- --include-ignored
cargo test -p chronon-scheduler --test leader_integration
cargo test -p chronon-testkit -- --test-threads=1

echo "== Step 3: Extended postgres/postgres-redis matrix =="
cargo test -p chronon-e2e --test scenarios -- --ignored --test-threads=1

echo "== Step 4: Multi-process distributed smoke =="
pkill -f 'target/debug/examples/coordinator_daemon' 2>/dev/null || true
pkill -f 'target/debug/examples/worker_daemon' 2>/dev/null || true
cargo build -q -p uf-chronon --example coordinator_daemon --example worker_daemon --features postgres,redis
cargo test -p chronon-e2e --test distributed_smoke -- --ignored --test-threads=1

echo "AWS E2E gate: PASS"
