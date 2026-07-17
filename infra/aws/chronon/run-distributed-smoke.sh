#!/usr/bin/env bash
# Start coordinator + two workers on remote hosts (SSM) or local mode for distributed_smoke test.
set -euo pipefail

MODE="${CHRONON_DISTRIBUTED_MODE:-remote}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$ROOT"

export CARGO_BUILD_JOBS=1

if [[ "$MODE" == "local" ]]; then
  export CHRONON_DISTRIBUTED_MODE=local
  cargo test -p chronon-e2e --test distributed_smoke -- --ignored --test-threads=1 --nocapture
  exit 0
fi

# Remote mode: daemons must already be running on coordinator/worker instances.
export CHRONON_DISTRIBUTED_MODE=remote
cargo test -p chronon-e2e --test distributed_smoke -- --ignored --test-threads=1 --nocapture
