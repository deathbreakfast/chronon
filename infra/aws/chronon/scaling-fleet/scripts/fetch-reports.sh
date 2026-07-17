#!/usr/bin/env bash
# SCP scaling-fleet reports to local profiling directory.
set -euo pipefail

SF="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT="$(cd "$SF/../../../.." && pwd)"
# shellcheck disable=SC1091
source "$SF/lib/bench-fleet.sh"

SSH_KEY="${CHRONON_SSH_KEY:?set CHRONON_SSH_KEY to the SSH private key path}"
SSH_OPTS=(-o StrictHostKeyChecking=no -o ConnectTimeout=30 -i "$SSH_KEY")
HOST="$(resolve_bench_ip 1)"
LOCAL="$ROOT/profiling/chronon-bench/reports"

mkdir -p "$LOCAL"
rsync -az -e "ssh ${SSH_OPTS[*]}" \
  "ec2-user@${HOST}:~/chronon/profiling/chronon-bench/reports/" \
  "$LOCAL/"

echo "synced reports to $LOCAL"
