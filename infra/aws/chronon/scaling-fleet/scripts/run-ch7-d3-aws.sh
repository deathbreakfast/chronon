#!/usr/bin/env bash
# CH7-D3: multibench bc ∈ {1,2,4} with START_EPOCH synchronization.
set -euo pipefail

SF="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck disable=SC1091
source "$SF/lib/bench-fleet.sh"
eval "$("$SF/export-fleet-env.sh")"

SSH_KEY="$(bench_ssh_key)"
SSH_OPTS=(-o StrictHostKeyChecking=no -o ConnectTimeout=30 -i "$SSH_KEY")
BENCH="${CHRONON_BENCH_BIN:-~/chronon-bench/bin/chronon-bench}"
BENCH_COUNT="${BENCH_COUNT:-4}"
PREFILL="${CHRONON_BENCH_PREFILL:-100000}"
W="${CHRONON_BENCH_WORKER_COUNT:-16}"
STORAGE="${CHRONON_BENCH_STORAGE:-postgres-redis}"
BC_VALUES="${CHRONON_CH7_MULTIBENCH_BC:-1 2 4}"
CPU_LOG_DIR="/tmp/chronon-d3-cpu"
CPU_PIDS=()

start_cpu_sampling() {
  local label="$1"
  local host="$2"
  ssh "${SSH_OPTS[@]}" "ec2-user@${host}" "mkdir -p ${CPU_LOG_DIR} && (command -v mpstat >/dev/null && mpstat 1 > ${CPU_LOG_DIR}/${label}.log || top -b -d 1 > ${CPU_LOG_DIR}/${label}.log) & echo \$! > ${CPU_LOG_DIR}/${label}.pid" || true
}

stop_cpu_sampling() {
  local label="$1"
  local host="$2"
  ssh "${SSH_OPTS[@]}" "ec2-user@${host}" "[[ -f ${CPU_LOG_DIR}/${label}.pid ]] && kill \$(cat ${CPU_LOG_DIR}/${label}.pid) 2>/dev/null || true" || true
}

echo "skipping CPU sampling (SSH/mpstat hangs on fleet hosts)"
data_host="$(resolve_data_ssh_host)"

collect_cell_reports() {
  local bc="$1"
  local tag="$2"
  local bench0
  bench0="$(resolve_bench_ip 1)"
  for i in $(seq 2 "$bc"); do
    local host idx fname
    host="$(resolve_bench_ip "$i")"
    idx=$((i - 1))
    fname="${tag}-i${idx}-${STORAGE}-${CHRONON_BENCH_HARDWARE}.json"
    scp "${SSH_OPTS[@]}" "ec2-user@${host}:~/chronon/profiling/chronon-bench/reports/${fname}"       "ec2-user@${bench0}:~/chronon/profiling/chronon-bench/reports/${fname}"
  done
}

run_cell() {
  local bc="$1"
  local w_per_host=$(( W / bc ))
  [[ "$w_per_host" -ge 1 ]] || w_per_host=1
  local tag="bm-ch7-bc${bc}-w${w_per_host}-q${PREFILL}"
  local pids=()

  START_EPOCH=$(( $(date +%s) + 120 ))
  echo "=== D3 cell bc=${bc} tag=${tag} START_EPOCH=${START_EPOCH} ==="

  for i in $(seq 0 $((bc - 1))); do
    local host_idx=$((i + 1))
    local host
    host="$(resolve_bench_ip "$host_idx")"
    local drain_env=""
    if [[ "$i" -gt 0 ]]; then
      drain_env="export CHRONON_BENCH_DRAIN_ONLY=1 &&"
    fi
    ssh "${SSH_OPTS[@]}" "ec2-user@${host}" \
      "export START_EPOCH=${START_EPOCH} && \
       export CHRONON_BENCH_CELL_ID='${tag}' && \
       while [[ \$(date +%s) -lt \$START_EPOCH ]]; do sleep 1; done && \
       ${drain_env} \
       export CHRONON_POSTGRES_URL='${CHRONON_POSTGRES_URL}' && \
       export CHRONON_REDIS_URL='${CHRONON_REDIS_URL}' && \
       export CHRONON_BENCH_CLIENT_INDEX=${i} && \
       export CHRONON_BENCH_CLIENT_COUNT=${bc} && \
       export CHRONON_BENCH_HARDWARE='${CHRONON_BENCH_HARDWARE}' && \
       mkdir -p ~/chronon/profiling/chronon-bench/reports && \
       ${BENCH} run \
         --experiment bm-ch7 \
         --storage ${STORAGE} \
         --worker-count ${w_per_host} \
         --prefill ${PREFILL} \
         --bench-client-index ${i} \
         --bench-client-count ${bc} \
         --hardware ${CHRONON_BENCH_HARDWARE} \
         --report ~/chronon/profiling/chronon-bench/reports/${tag}-i${i}-${STORAGE}-${CHRONON_BENCH_HARDWARE}.json" &
    pids+=($!)
  done

  for pid in "${pids[@]}"; do wait "$pid" || true; done

  if [[ "$bc" -gt 1 ]]; then
    collect_cell_reports "$bc" "$tag"

    local host
    host="$(resolve_bench_ip 1)"
    ssh "${SSH_OPTS[@]}" "ec2-user@${host}" bash -s <<EOF
set -euo pipefail
${BENCH} aggregate \
  --storage ${STORAGE} \
  --hardware ${CHRONON_BENCH_HARDWARE} \
  --reports-dir ~/chronon/profiling/chronon-bench/reports \
  --cell-prefix ${tag}
EOF
  fi
}

for bc in $BC_VALUES; do
  if [[ "$bc" -gt "$BENCH_COUNT" ]]; then
    echo "skip bc=${bc}: BENCH_COUNT=${BENCH_COUNT}" >&2
    continue
  fi
  run_cell "$bc"
done

host="$(resolve_bench_ip 1)"
ssh "${SSH_OPTS[@]}" "ec2-user@${host}" bash -s <<EOF
set -euo pipefail
${BENCH} scaling-curve ch7-multibench-curve \
  --storage ${STORAGE} \
  --hardware ${CHRONON_BENCH_HARDWARE} \
  --reports-dir ~/chronon/profiling/chronon-bench/reports
EOF

"$SF/scripts/fetch-reports.sh"
