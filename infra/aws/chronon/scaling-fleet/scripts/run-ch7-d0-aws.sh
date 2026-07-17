#!/usr/bin/env bash
# Run CH7-D0 on remote bench host (single-host authoritative proxy).
set -euo pipefail

SF="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT="$(cd "$SF/../../.." && pwd)"
# shellcheck disable=SC1091
source "$SF/lib/bench-fleet.sh"
eval "$("$SF/export-fleet-env.sh")"

SSH_KEY="${CHRONON_SSH_KEY:?set CHRONON_SSH_KEY to the SSH private key path}"
SSH_OPTS=(-o StrictHostKeyChecking=no -o ConnectTimeout=30 -i "$SSH_KEY")
HOST="$(resolve_bench_ip 1)"

ssh "${SSH_OPTS[@]}" "ec2-user@${HOST}" bash -s <<EOF
set -euo pipefail
export CHRONON_POSTGRES_URL=${CHRONON_POSTGRES_URL}
export CHRONON_REDIS_URL=${CHRONON_REDIS_URL}
export CHRONON_BENCH_HARDWARE=${CHRONON_BENCH_HARDWARE}
export CARGO_BUILD_JOBS=1
export CARGO_TARGET_DIR=target-chronon-bench
source ~/.cargo/env 2>/dev/null || true
cd ~/chronon
./chronon-bench/scripts/run-ch7-d0-worker-sweep.sh
EOF

"$SF/scripts/fetch-reports.sh"
