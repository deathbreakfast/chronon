#!/usr/bin/env bash
# T5: multi-cell postgres-redis fleet (10k gate).
set -euo pipefail

SF="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck disable=SC1091
source "$SF/lib/tier-common.sh"

export CHRONON_BENCH_TIER=T5
export CHRONON_DATA_TIER_PROFILE=multicell-t5

echo "=== T5 multi-cell fleet $(date -u +%Y-%m-%dT%H:%M:%SZ) ===" | tee -a "$TIER_LOG"

MAX_N="${CHRONON_T5_MAX_CELLS:-16}"
"$SF/provision-scaling-fleet-cells.sh" "$MAX_N" 2>&1 | tee -a "$TIER_LOG"

# Always provision fresh bench hosts matching cell count (fleet may have been torn down).
export CHRONON_BENCH_COUNT="$MAX_N"
"$SF/provision-scaling-fleet.sh" "bench-${MAX_N}" 2>&1 | tee -a "$TIER_LOG"
# shellcheck disable=SC1091
source "$SF/instances.env"

BENCH0="$(tier_bench_host)"
# shellcheck disable=SC2207
SSH_OPTS=($(tier_ssh_opts))

# Rebuild only when missing — private quark deps make every remote cargo expensive.
if [[ "${CHRONON_SKIP_REMOTE_BUILD:-}" == "1" ]] \
  && ssh "${SSH_OPTS[@]}" "ec2-user@${BENCH0}" 'test -x ~/chronon-bench/bin/chronon-bench'; then
  echo "reuse existing remote chronon-bench binary" | tee -a "$TIER_LOG"
else
  "$SF/deploy-bench-binary.sh" 2>&1 | tee -a "$TIER_LOG"
fi

# Fleet-local scripts live under ~/chronon/scaling-fleet (not the full repo tree).
ssh "${SSH_OPTS[@]}" "ec2-user@${BENCH0}" \
  'mkdir -p ~/chronon/scaling-fleet/scripts ~/chronon/scaling-fleet/lib ~/chronon/profiling/chronon-bench/reports ~/.ssh'
# Rewrite laptop key path so fleet-local SSH works on the remote host.
sed 's|^CHRONON_SSH_KEY=.*|CHRONON_SSH_KEY=$HOME/.ssh/chronon-bench.pem|' \
  "$SF/instances.env" > /tmp/chronon-t5-instances.env
scp "${SSH_OPTS[@]}" /tmp/chronon-t5-instances.env "ec2-user@${BENCH0}:~/chronon/scaling-fleet/instances.env"
scp "${SSH_OPTS[@]}" "$SF/instances.cells.env" "ec2-user@${BENCH0}:~/chronon/scaling-fleet/instances.cells.env"
scp "${SSH_OPTS[@]}" "$SF/lib/"*.sh "ec2-user@${BENCH0}:~/chronon/scaling-fleet/lib/"
scp "${SSH_OPTS[@]}" "$SF/scripts/run-ch7-t5-fleet-local.sh" "ec2-user@${BENCH0}:~/chronon/scaling-fleet/scripts/run-ch7-t5-fleet-local.sh"
scp "${SSH_OPTS[@]}" "$(bench_ssh_key)" "ec2-user@${BENCH0}:~/.ssh/chronon-bench.pem"

ssh "${SSH_OPTS[@]}" "ec2-user@${BENCH0}" bash -s <<'REMOTE' | tee -a "$TIER_LOG"
set -euo pipefail
chmod 600 ~/.ssh/chronon-bench.pem
export CHRONON_SSH_KEY=$HOME/.ssh/chronon-bench.pem
export CHRONON_BENCH_WORKERS_PER_HOST=1
export CHRONON_BENCH_PREFILL=100000
export CHRONON_T5_CELL_LADDER="1 2 4 8 16"
~/chronon/scaling-fleet/scripts/run-ch7-t5-fleet-local.sh
REMOTE

tier_finish T5
echo "T5 complete"
PROGRESS_FILE="${CHRONON_D5_PROGRESS_FILE:-$SF/../.state/d5-ladder-progress.txt}"
mkdir -p "$(dirname "$PROGRESS_FILE")"
echo T5 > "$PROGRESS_FILE"
