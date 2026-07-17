#!/usr/bin/env bash
# CH7 D5 full experiment ladder T0–T7 (postgres-redis, sync PG).
# Single autorun: provision → matrix → fetch → verify → progress log per tier.
# Fails if any tier script exits non-zero.
set -euo pipefail

SF="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPTS="$SF/scripts"
# shellcheck disable=SC1091
source "$SF/lib/tier-common.sh"

export CHRONON_SKIP_BENCH_PROVISION="${CHRONON_SKIP_BENCH_PROVISION:-}"
TIER_LOG="${CHRONON_D5_LOG:-/tmp/chronon-d5-ladder.log}"
LADDER_PIDFILE="/tmp/chronon-d5-ladder.pid"

if [[ -f "$LADDER_PIDFILE" ]]; then
  existing="$(cat "$LADDER_PIDFILE")"
  if [[ "$existing" != "$$" ]] && kill -0 "$existing" 2>/dev/null; then
    echo "ladder already running pid=${existing}" | tee -a "$TIER_LOG"
    exit 1
  fi
fi
echo $$ > "$LADDER_PIDFILE"
trap 'rm -f "$LADDER_PIDFILE"' EXIT

mkdir -p "$(dirname "$TIER_PROGRESS")" "$(dirname "$TIER_METRICS")"
{
  echo ""
  echo "## D5 full ladder start $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "- autorun: \`run-ch7-d5-full-ladder-aws.sh\`"
  echo "- log: \`${TIER_LOG}\`"
} >> "$TIER_PROGRESS"

run_tier() {
  local tier="$1"
  local script="$2"
  echo "========== ${tier} ==========" | tee -a "$TIER_LOG"
  if [[ ! -x "$script" ]]; then
    chmod +x "$script"
  fi
  "$script" || {
    echo "TIER ${tier} FAILED" | tee -a "$TIER_LOG"
    exit 1
  }
}

should_run_tier() {
  local tier="$1"
  local start="${CHRONON_D5_START_TIER:-T0}"
  local -a order=(T0 T1 T2 T3 T4 T5 T6 T7)
  local seen=0
  for t in "${order[@]}"; do
    [[ "$t" == "$start" ]] && seen=1
    [[ "$t" == "$tier" && "$seen" -eq 1 ]] && return 0
  done
  return 1
}

run_tier_if() {
  local tier="$1"
  local script="$2"
  if should_run_tier "$tier"; then
    run_tier "$tier" "$script"
  else
    echo "skip ${tier} (CHRONON_D5_START_TIER=${CHRONON_D5_START_TIER:-T0})" | tee -a "$TIER_LOG"
  fi
}

# Ensure VPC + data services exist.
"$SF/ensure-data-services.sh" 2>&1 | tee -a "$TIER_LOG"

# Skip early deploy when resuming at T5+ — those tiers provision fresh hosts first.
start_tier="${CHRONON_D5_START_TIER:-T0}"
if [[ "$start_tier" =~ ^T(5|6|7)$ ]]; then
  echo "skip early deploy (start=${start_tier}; tier will provision + deploy)" | tee -a "$TIER_LOG"
elif [[ "${CHRONON_SKIP_REMOTE_BUILD:-}" == "1" ]]; then
  echo "reuse deployed bench binary (CHRONON_SKIP_REMOTE_BUILD=1)" | tee -a "$TIER_LOG"
else
  tier_deploy_bench
  export CHRONON_SKIP_REMOTE_BUILD=1
fi

run_tier_if T0 "$SCRIPTS/run-ch7-t0-matrix.sh"
run_tier_if T1 "$SCRIPTS/run-ch7-t1-split.sh"
run_tier_if T2 "$SCRIPTS/run-ch7-t2-sized.sh"
run_tier_if T3 "$SCRIPTS/run-ch7-t3-cluster.sh"
run_tier_if T4 "$SCRIPTS/run-ch7-t4-pg-scale.sh"
run_tier_if T5 "$SCRIPTS/run-ch7-t5-multicell.sh"
run_tier_if T6 "$SCRIPTS/run-ch7-t6-batch.sh"
run_tier_if T7 "$SCRIPTS/run-ch7-t7-ch7d.sh"

{
  echo ""
  echo "## D5 full ladder COMPLETE $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "- all tiers T0–T7 executed"
} >> "$TIER_PROGRESS"

if [[ "${CHRONON_D5_TEARDOWN_ON_COMPLETE:-1}" == "1" ]]; then
  echo "=== teardown fleet $(date -u +%Y-%m-%dT%H:%M:%SZ) ===" | tee -a "$TIER_LOG"
  "$SF/teardown-chronon-fleet.sh" 2>&1 | tee -a "$TIER_LOG" || true
fi

echo "D5 full ladder complete — see ${TIER_PROGRESS} and ${TIER_LOG}"
