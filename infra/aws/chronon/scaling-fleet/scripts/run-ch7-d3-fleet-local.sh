#!/usr/bin/env bash
# CH7-D3 multibench — runs on bench_0, orchestrates all bench hosts via private IP.
# Laptop-free: nohup this script on bench_0 after deploy.
set -euo pipefail

LOG="${CHRONON_D3_LOG:-$HOME/chronon-d3-autorun.log}"
exec > >(tee -a "$LOG") 2>&1

echo "=== chronon D3 fleet-local autorun $(date -u +%Y-%m-%dT%H:%M:%SZ) ==="

SF="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck disable=SC1091
source "${SF}/instances.env"

BENCH="${CHRONON_BENCH_BIN:-$HOME/chronon-bench/bin/chronon-bench}"
PREFILL="${CHRONON_BENCH_PREFILL:-100000}"
W_PER_HOST="${CHRONON_BENCH_WORKERS_PER_HOST:-${CHRONON_BENCH_WORKER_COUNT:-1}}"
STORAGE="${CHRONON_BENCH_STORAGE:-postgres-redis}"
BC_VALUES="${CHRONON_CH7_MULTIBENCH_BC:-1 2 4}"
HARDWARE="${CHRONON_BENCH_HARDWARE:-aws-c6i-large}"
POOL_LAYOUT="${CHRONON_BENCH_POOL_LAYOUT:-distinct}"

PG_URL="postgres://chronon:chronon@${POSTGRES_IP}:5432/chronon"
RD_URL="redis://${REDIS_IP}:6379"

SSH_KEY="${CHRONON_SSH_KEY:?set CHRONON_SSH_KEY to the SSH private key path}"
SSH_OPTS=(-o StrictHostKeyChecking=no -o ConnectTimeout=30 -i "$SSH_KEY")

bench_priv_ip() {
  local idx="$1"
  local var="BENCH_$((idx - 1))_IP"
  echo "${!var}"
}

collect_cell_reports() {
  local bc="$1"
  local tag="$2"
  local bench0
  bench0="$(bench_priv_ip 1)"
  for i in $(seq 2 "$bc"); do
    local host idx fname
    host="$(bench_priv_ip "$i")"
    idx=$((i - 1))
    fname="${tag}-i${idx}-${STORAGE}-${HARDWARE}.json"
    scp "${SSH_OPTS[@]}" "ec2-user@${host}:~/chronon/profiling/chronon-bench/reports/${fname}" \
      "ec2-user@${bench0}:~/chronon/profiling/chronon-bench/reports/${fname}"
  done
}

verify_reports() {
  local bc="$1"
  local tag="$2"
  local start_epoch="$3"
  local reports="$HOME/chronon/profiling/chronon-bench/reports"
  local fail=0
  if [[ "$bc" -le 1 ]]; then
    return 0
  fi
  for idx in $(seq 1 $((bc - 1))); do
    local f="${reports}/${tag}-i${idx}-${STORAGE}-${HARDWARE}.json"
    if [[ ! -f "$f" ]]; then
      echo "VERIFY FAIL: missing $f" >&2
      fail=1
      continue
    fi
    python3 - "$f" "$start_epoch" "$idx" <<'PY' || fail=1
import json, re, sys
from datetime import datetime
path, start_epoch, idx = sys.argv[1], int(sys.argv[2]), sys.argv[3]
rep = json.load(open(path))
ops = rep.get("ops") or 0
recorded = rep.get("recorded_at") or ""
if ops <= 0:
    print(f"VERIFY FAIL: drain-only client {idx} ops={ops} in {path}", file=sys.stderr)
    sys.exit(1)
if recorded:
    # Python 3.9 on AL2023 rejects >6 fractional digits in fromisoformat.
    iso = re.sub(r"(\.\d{6})\d+", r"\1", recorded.replace("Z", "+00:00"))
    ts = datetime.fromisoformat(iso)
    if ts.timestamp() < start_epoch - 30:
        print(f"VERIFY FAIL: stale report for client {idx} recorded_at={recorded}", file=sys.stderr)
        sys.exit(1)
PY
  done
  return "$fail"
}

clear_cell_reports() {
  local bc="$1"
  local tag="$2"
  for host_idx in $(seq 1 "$bc"); do
    local host
    host="$(bench_priv_ip "$host_idx")"
    ssh "${SSH_OPTS[@]}" "ec2-user@${host}" \
      "rm -f ~/chronon/profiling/chronon-bench/reports/${tag}-i*-${STORAGE}-${HARDWARE}.json" || true
  done
}

run_cell() {
  local bc="$1"
  local w_per_host="$W_PER_HOST"
  local pool_count="${CHRONON_BENCH_POOL_COUNT:-$bc}"
  local tag="bm-ch7-bc${bc}-w${w_per_host}-q${PREFILL}-k${pool_count}"
  local pids=()

  clear_cell_reports "$bc" "$tag"

  START_EPOCH=$(( $(date +%s) + 120 ))
  echo "=== D3 cell bc=${bc} tag=${tag} START_EPOCH=${START_EPOCH} ==="

  for i in $(seq 0 $((bc - 1))); do
    local host_idx=$((i + 1))
    local host
    host="$(bench_priv_ip "$host_idx")"
    local drain_env=""
    if [[ "$i" -gt 0 ]]; then
      drain_env="export CHRONON_BENCH_DRAIN_ONLY=1 &&"
    fi
    ssh "${SSH_OPTS[@]}" "ec2-user@${host}" \
      "export START_EPOCH=${START_EPOCH} && \
       export CHRONON_BENCH_CELL_ID='${tag}' && \
       export CHRONON_POSTGRES_URL='${PG_URL}' && \
       export CHRONON_REDIS_URL='${RD_URL}' && \
       export CHRONON_BENCH_CLIENT_INDEX=${i} && \
       export CHRONON_BENCH_CLIENT_COUNT=${bc} && \
       export CHRONON_BENCH_HARDWARE='${HARDWARE}' && \
       while [[ \$(date +%s) -lt \$START_EPOCH ]]; do sleep 1; done && \
       ${drain_env} \
       mkdir -p ~/chronon/profiling/chronon-bench/reports && \
       ${BENCH} run \
         --experiment bm-ch7 \
         --storage ${STORAGE} \
         --worker-count ${w_per_host} \
         --pool-count ${pool_count} \
         --pool-layout ${POOL_LAYOUT} \
         --prefill ${PREFILL} \
         --bench-client-index ${i} \
         --bench-client-count ${bc} \
         --hardware ${HARDWARE} \
         --report ~/chronon/profiling/chronon-bench/reports/${tag}-i${i}-${STORAGE}-${HARDWARE}.json" &
    pids+=($!)
  done

  for pid in "${pids[@]}"; do wait "$pid" || true; done

  if [[ "$bc" -gt 1 ]]; then
    collect_cell_reports "$bc" "$tag"
    verify_reports "$bc" "$tag" "$START_EPOCH" || return 1
    ${BENCH} aggregate \
      --storage "${STORAGE}" \
      --hardware "${HARDWARE}" \
      --reports-dir ~/chronon/profiling/chronon-bench/reports \
      --cell-prefix "${tag}"
  fi
}

mkdir -p ~/chronon/profiling/chronon-bench/reports

for bc in $BC_VALUES; do
  if [[ "$bc" -gt "${BENCH_COUNT:-4}" ]]; then
    echo "skip bc=${bc}: BENCH_COUNT=${BENCH_COUNT:-4}" >&2
    continue
  fi
  run_cell "$bc"
done

${BENCH} scaling-curve ch7-multibench-curve \
  --storage "${STORAGE}" \
  --hardware "${HARDWARE}" \
  --reports-dir ~/chronon/profiling/chronon-bench/reports

echo "=== D3 fleet-local COMPLETE $(date -u +%Y-%m-%dT%H:%M:%SZ) ==="
echo "Reports: ~/chronon/profiling/chronon-bench/reports/"
ls -la ~/chronon/profiling/chronon-bench/reports/bm-ch7-bc*-q${PREFILL}-*.json 2>/dev/null || true
