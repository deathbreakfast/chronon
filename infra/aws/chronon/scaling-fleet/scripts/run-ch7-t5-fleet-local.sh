#!/usr/bin/env bash
# T5 multi-cell fleet — runs on bench_0, one primary-row client per cell.
set -euo pipefail

LOG="${CHRONON_T5_LOG:-$HOME/chronon-t5-autorun.log}"
exec > >(tee -a "$LOG") 2>&1

SF="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck disable=SC1091
source "${SF}/instances.env"
# shellcheck disable=SC1091
source "${SF}/instances.cells.env"

BENCH="${CHRONON_BENCH_BIN:-$HOME/chronon-bench/bin/chronon-bench}"
PREFILL="${CHRONON_BENCH_PREFILL:-100000}"
W_PER_HOST="${CHRONON_BENCH_WORKERS_PER_HOST:-1}"
HARDWARE="${CHRONON_BENCH_HARDWARE:-aws-c6i-large}"
STORAGE="${CHRONON_BENCH_STORAGE:-postgres-redis}"
N_CELLS="${1:-${CELL_COUNT:-4}}"

# Prefer the key path already staged on this host (or CHRONON_SSH_KEY).
if [[ -n "${CHRONON_SSH_KEY:-}" && -f "${CHRONON_SSH_KEY}" ]]; then
  SSH_KEY="${CHRONON_SSH_KEY}"
elif [[ -f "$HOME/.ssh/chronon-bench.pem" ]]; then
  SSH_KEY="$HOME/.ssh/chronon-bench.pem"
else
  echo "set CHRONON_SSH_KEY or stage ~/.ssh/chronon-bench.pem on this host" >&2
  exit 1
fi
chmod 600 "$SSH_KEY" 2>/dev/null || true
SSH_OPTS=(-o StrictHostKeyChecking=no -o ConnectTimeout=30 -i "$SSH_KEY")

bench_priv_ip() {
  local idx="$1"
  local var="BENCH_$((idx - 1))_IP"
  echo "${!var}"
}

run_fleet_n() {
  local n="$1"
  local tag="bm-ch7-t5-n${n}-w${W_PER_HOST}-q${PREFILL}-k${n}"
  local pids=()
  START_EPOCH=$(( $(date +%s) + 120 ))
  echo "=== T5 fleet N=${n} tag=${tag} START=${START_EPOCH} ==="

  for i in $(seq 0 $((n - 1))); do
    local host_idx=$((i + 1))
    local host
    host="$(bench_priv_ip "$host_idx")"
    local pg_var="CELL_${i}_POSTGRES_IP"
    local rd_var="CELL_${i}_REDIS_IP"
    local prefix_var="CELL_${i}_KEY_PREFIX"
    local pg_url="postgres://chronon:chronon@${!pg_var}:5432/chronon"
    local rd_url="redis://${!rd_var}:6379"
    local prefix="${!prefix_var:-cell${i}}"
    # Each cell has its own PG+Redis — run standalone (client_count=1). Using
    # multibench client_count=N makes clients wait on a shared schema barrier
    # that never completes across isolated databases.
    ssh "${SSH_OPTS[@]}" "ec2-user@${host}" \
      "export START_EPOCH=${START_EPOCH} && \
       export CHRONON_POSTGRES_URL='${pg_url}' && \
       export CHRONON_REDIS_URL='${rd_url}' && \
       export CHRONON_BENCH_TIER=T5 && \
       export CHRONON_DATA_TIER_PROFILE=multicell-t5 && \
       export CHRONON_BENCH_CELL_INDEX=${i} && \
       export CHRONON_KEY_PREFIX='${prefix}' && \
       export CHRONON_CLAIM_BATCH='${CHRONON_CLAIM_BATCH:-}' && \
       while [[ \$(date +%s) -lt \$START_EPOCH ]]; do sleep 1; done && \
       mkdir -p ~/chronon/profiling/chronon-bench/reports && \
       ${BENCH} run \
         --experiment bm-ch7 \
         --storage ${STORAGE} \
         --worker-count ${W_PER_HOST} \
         --pool-count 1 \
         --prefill ${PREFILL} \
         --bench-client-index 0 \
         --bench-client-count 1 \
         --hardware ${HARDWARE} \
         --report ~/chronon/profiling/chronon-bench/reports/${tag}-cell${i}-${STORAGE}-${HARDWARE}.json" &
    pids+=($!)
  done

  for pid in "${pids[@]}"; do wait "$pid" || true; done

  local bench0
  bench0="$(bench_priv_ip 1)"
  # Collect per-cell reports onto bench_0 (cell i runs on bench i+1).
  for i in $(seq 1 $((n - 1))); do
    local host fname
    host="$(bench_priv_ip $((i + 1)))"
    fname="${tag}-cell${i}-${STORAGE}-${HARDWARE}.json"
    scp "${SSH_OPTS[@]}" "ec2-user@${host}:~/chronon/profiling/chronon-bench/reports/${fname}" \
      "${HOME}/chronon/profiling/chronon-bench/reports/${fname}" || true
  done

  python3 - "$n" "$tag" "$STORAGE" "$HARDWARE" <<'PY'
import json, glob, sys
from pathlib import Path
n, tag, storage, hw = sys.argv[1:5]
reports = Path.home() / "chronon/profiling/chronon-bench/reports"
paths = sorted(reports.glob(f"{tag}-cell*-{storage}-{hw}.json"))
wall_sum = 0.0
cells = []
for p in paths[: int(n)]:
    rep = json.loads(p.read_text())
    wall = rep.get("fleet_wall_claim_ops_per_sec") or rep.get("claim_ops_per_sec") or 0.0
    wall_sum += float(wall)
    cells.append({"path": p.name, "wall": wall})
out = {
    "experiment": "bm-ch7-t5-fleet",
    "cell_count": int(n),
    "fleet_wall_claim_ops_per_sec": wall_sum,
    "cells": cells,
    "tier_tag": "T5",
    "data_tier_profile": "multicell-t5",
}
out_path = reports / f"{tag}-aggregate-{storage}-{hw}.json"
out_path.write_text(json.dumps(out, indent=2) + "\n")
print(f"T5 N={n} fleet_wall={wall_sum:.1f}/s -> {out_path}")
if wall_sum >= 10000:
    print("10K GATE PASS")
elif wall_sum <= 0:
    print("T5 ZERO THROUGHPUT — check SSH key / cell connectivity", file=sys.stderr)
    sys.exit(1)
PY
}

mkdir -p ~/chronon/profiling/chronon-bench/reports
for n in ${CHRONON_T5_CELL_LADDER:-1 2 4 8 16}; do
  if [[ "$n" -gt "${BENCH_COUNT:-4}" ]] || [[ "$n" -gt "${CELL_COUNT:-0}" ]]; then
    echo "skip N=${n}: need bench=${BENCH_COUNT} cells=${CELL_COUNT}"
    continue
  fi
  run_fleet_n "$n"
done

echo "=== T5 fleet-local COMPLETE $(date -u +%Y-%m-%dT%H:%M:%SZ) ==="
