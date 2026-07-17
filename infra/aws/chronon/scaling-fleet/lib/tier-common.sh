#!/usr/bin/env bash
# Shared helpers for CH7 D5 tier ladder campaigns.
set -euo pipefail

TIER_SF="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TIER_ROOT="$(cd "$TIER_SF/../../../.." && pwd)"
TIER_SCRIPTS="$TIER_SF/scripts"
TIER_STATE="${CHRONON_AWS_STATE_DIR:-$TIER_ROOT/infra/aws/chronon/.state}"
TIER_METRICS="${CHRONON_HYPERSCALE_METRICS:-$TIER_STATE/hyperscale-metrics.log}"
TIER_PROGRESS="${CHRONON_HYPERSCALE_PROGRESS:-$TIER_STATE/bench-hyperscale-progress.md}"
TIER_LOG="${CHRONON_D5_LOG:-/tmp/chronon-d5-ladder.log}"
mkdir -p "$TIER_STATE"

# shellcheck disable=SC1091
source "$TIER_SF/lib/bench-fleet.sh"

tier_ssh_opts() {
  echo "-o StrictHostKeyChecking=no -o ConnectTimeout=30 -i $(bench_ssh_key)"
}

tier_bench_host() {
  resolve_bench_ip 1
}

tier_eval_fleet_env() {
  # Preserve caller overrides (e.g. T6 pointing at a multicell host).
  local saved_pg="${CHRONON_POSTGRES_URL:-}"
  local saved_rd="${CHRONON_REDIS_URL:-}"
  eval "$("$TIER_SF/export-fleet-env.sh")"
  if [[ -n "$saved_pg" ]]; then
    export CHRONON_POSTGRES_URL="$saved_pg"
  fi
  if [[ -n "$saved_rd" ]]; then
    export CHRONON_REDIS_URL="$saved_rd"
  fi
}

tier_deploy_bench() {
  echo "=== deploy bench binary $(date -u +%Y-%m-%dT%H:%M:%SZ) ===" | tee -a "$TIER_LOG"
  local host
  host="$(tier_bench_host)"
  local -a SSH_OPTS
  # shellcheck disable=SC2207
  SSH_OPTS=($(tier_ssh_opts))

  if [[ "${CHRONON_SKIP_REMOTE_BUILD:-}" == "1" ]]; then
    echo "skip remote build (CHRONON_SKIP_REMOTE_BUILD=1)" | tee -a "$TIER_LOG"
  elif ! "$TIER_SF/deploy-bench-binary.sh" 2>&1 | tee -a "$TIER_LOG"; then
    if ssh "${SSH_OPTS[@]}" "ec2-user@${host}" test -x ~/chronon-bench/bin/chronon-bench; then
      echo "remote build failed; reusing existing ~/chronon-bench/bin/chronon-bench on bench_0" | tee -a "$TIER_LOG"
      rsync -az \
        --exclude 'target' --exclude 'target-*' --exclude '.git' --exclude 'profiling' \
        -e "ssh ${SSH_OPTS[*]}" \
        "$TIER_ROOT/infra/" "ec2-user@${host}:~/chronon/infra/" 2>&1 | tee -a "$TIER_LOG" || true
    else
      echo "remote build failed and no bench binary on bench_0" | tee -a "$TIER_LOG"
      return 1
    fi
  fi
}

tier_run_bench() {
  local label="$1"
  local report_name="$2"
  shift 2
  local remote_cmd="$*"
  local host
  host="$(tier_bench_host)"
  tier_eval_fleet_env
  local -a SSH_OPTS
  # shellcheck disable=SC2207
  SSH_OPTS=($(tier_ssh_opts))

  echo "=== RUN ${label} $(date -u +%Y-%m-%dT%H:%M:%SZ) ===" | tee -a "$TIER_LOG"
  "$TIER_SCRIPTS/capture-data-tier-metrics.sh" "$label" "$TIER_METRICS" &
  local mpid=$!
  ssh "${SSH_OPTS[@]}" "ec2-user@${host}" bash -s <<EOF | tee -a "$TIER_LOG"
set -euo pipefail
export CHRONON_POSTGRES_URL='${CHRONON_POSTGRES_URL}'
export CHRONON_REDIS_URL='${CHRONON_REDIS_URL}'
export CHRONON_BENCH_HARDWARE='${CHRONON_BENCH_HARDWARE:-aws-c6i-large}'
export CHRONON_BENCH_STORAGE=postgres-redis
export CHRONON_BENCH_TIER='${CHRONON_BENCH_TIER:-}'
export CHRONON_DATA_TIER_PROFILE='${CHRONON_DATA_TIER_PROFILE:-}'
export CHRONON_PG_POOL_SIZE='${CHRONON_PG_POOL_SIZE:-}'
export CHRONON_CLAIM_BATCH='${CHRONON_CLAIM_BATCH:-}'
export CHRONON_REDIS_HASH_TAGS='${CHRONON_REDIS_HASH_TAGS:-}'
export CHRONON_REDIS_CLUSTER_URLS='${CHRONON_REDIS_CLUSTER_URLS:-}'
export PATH=\$HOME/chronon-bench/bin:\$PATH
BENCH=\$HOME/chronon-bench/bin/chronon-bench
mkdir -p ~/chronon/profiling/chronon-bench/reports
${remote_cmd}
EOF
  wait "$mpid" || true
}

tier_ch7_run() {
  local label="$1"
  local report="$2"
  local w="$3"
  local k="$4"
  local q="$5"
  tier_run_bench "$label" "$report" \
    "\$BENCH run --experiment bm-ch7 --storage postgres-redis --worker-count ${w} --prefill ${q} \
      --pool-count ${k} --pool-layout distinct --hardware \${CHRONON_BENCH_HARDWARE} \
      --storage-topology \${CHRONON_DATA_TIER_PROFILE:-colocated} \
      --report ~/chronon/profiling/chronon-bench/reports/${report}"
}

tier_finish() {
  local tier="$1"
  echo "=== tier ${tier} fetch $(date -u +%Y-%m-%dT%H:%M:%SZ) ===" | tee -a "$TIER_LOG"
  "$TIER_SCRIPTS/fetch-reports.sh" 2>&1 | tee -a "$TIER_LOG"
  "$TIER_SCRIPTS/verify-authoritative-reports.sh" "$TIER_ROOT/profiling/chronon-bench/reports" 2>&1 | tee -a "$TIER_LOG" || true
  {
    echo ""
    echo "### ${tier} complete $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "- log: \`${TIER_LOG}\`"
    echo "- metrics: \`${TIER_METRICS}\`"
  } >> "$TIER_PROGRESS"
  # Durable resume marker (survives /tmp wipe; kept under ignored .state/).
  local progress_file="${CHRONON_D5_PROGRESS_FILE:-$TIER_STATE/d5-ladder-progress.txt}"
  mkdir -p "$(dirname "$progress_file")"
  echo "$tier" > "$progress_file"
  echo "${tier} complete" | tee -a "$TIER_LOG"
}

tier_provision_bench_d3() {
  if [[ "${CHRONON_SKIP_BENCH_PROVISION:-}" == "1" ]]; then
    echo "skip bench provision (CHRONON_SKIP_BENCH_PROVISION=1)" | tee -a "$TIER_LOG"
    return 0
  fi
  if [[ "${BENCH_COUNT:-0}" -ge 4 ]]; then
    echo "reuse existing ${BENCH_COUNT} bench hosts" | tee -a "$TIER_LOG"
    return 0
  fi
  "$TIER_SF/provision-scaling-fleet.sh" d3 2>&1 | tee -a "$TIER_LOG"
}

tier_write_instances_env_colocated() {
  local data_priv="$1"
  local data_pub="$2"
  local profile="${3:-colocated-t3}"
  cat > "$TIER_SF/instances.env" <<EOF
REGION=${AWS_REGION:-us-west-2}
BENCH_COUNT=${BENCH_COUNT:-4}
DATA_IP=${data_priv}
DATA_PUBLIC_IP=${data_pub}
POSTGRES_IP=${data_priv}
REDIS_IP=${data_priv}
STORAGE_TOPOLOGY=postgres-redis-colocated
DATA_TIER_PROFILE=${profile}
DATA_HOST=ec2-user@${data_pub}
$(for i in 0 1 2 3; do
  pub_var="BENCH_${i}_PUBLIC_IP"
  priv_var="BENCH_${i}_IP"
  if [[ -n "${!pub_var:-}" ]]; then
    echo "BENCH_${i}_IP=${!priv_var}"
    echo "BENCH_${i}_PUBLIC_IP=${!pub_var}"
  fi
done)
CHRONON_BENCH_INSTANCE_TYPE=${CHRONON_BENCH_INSTANCE_TYPE:-c6i.large}
CHRONON_DATA_INSTANCE_TYPE=${CHRONON_DATA_INSTANCE_TYPE:-t3.medium}
CHRONON_SSH_KEY=${CHRONON_SSH_KEY:?set CHRONON_SSH_KEY before writing instances.env}
PHASE=d5
EOF
}

tier_write_instances_env_split() {
  local redis_priv="$1"
  local redis_pub="$2"
  local pg_priv="$3"
  local pg_pub="$4"
  local profile="${5:-split-r6g}"
  cat > "$TIER_SF/instances.env" <<EOF
REGION=${AWS_REGION:-us-west-2}
BENCH_COUNT=${BENCH_COUNT:-4}
DATA_IP=${pg_priv}
DATA_PUBLIC_IP=${pg_pub}
POSTGRES_IP=${pg_priv}
REDIS_IP=${redis_priv}
REDIS_PUBLIC_IP=${redis_pub}
POSTGRES_PUBLIC_IP=${pg_pub}
STORAGE_TOPOLOGY=postgres-redis-split
DATA_TIER_PROFILE=${profile}
DATA_HOST=ec2-user@${pg_pub}
$(for i in 0 1 2 3; do
  pub_var="BENCH_${i}_PUBLIC_IP"
  priv_var="BENCH_${i}_IP"
  if [[ -n "${!pub_var:-}" ]]; then
    echo "BENCH_${i}_IP=${!priv_var}"
    echo "BENCH_${i}_PUBLIC_IP=${!pub_var}"
  fi
done)
CHRONON_BENCH_INSTANCE_TYPE=${CHRONON_BENCH_INSTANCE_TYPE:-c6i.large}
CHRONON_DATA_INSTANCE_TYPE=${CHRONON_DATA_INSTANCE_TYPE:-r6g.large}
CHRONON_SSH_KEY=${CHRONON_SSH_KEY:?set CHRONON_SSH_KEY before writing instances.env}
PHASE=d5
EOF
}
