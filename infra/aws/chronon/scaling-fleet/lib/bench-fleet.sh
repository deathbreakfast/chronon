#!/usr/bin/env bash
# Resolve bench host IP by 1-based index (public IP when available for laptop SSH).
set -euo pipefail

SF="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck disable=SC1091
source "${SF}/instances.env" 2>/dev/null || source "${SF}/instances.env.example"

# Path written onto remote bench hosts when scp'ing the operator key.
REMOTE_BENCH_SSH_KEY="${CHRONON_REMOTE_SSH_KEY:-$HOME/.ssh/chronon-bench.pem}"

require_ssh_key() {
  if [[ -z "${CHRONON_SSH_KEY:-}" || "${CHRONON_SSH_KEY}" == "REPLACE_ME" ]]; then
    echo "set CHRONON_SSH_KEY to the SSH private key path" >&2
    return 1
  fi
  if [[ ! -f "${CHRONON_SSH_KEY}" ]]; then
    echo "missing SSH key: ${CHRONON_SSH_KEY}" >&2
    return 1
  fi
}

resolve_bench_ip() {
  local idx="$1"
  local pub_var="BENCH_$((idx - 1))_PUBLIC_IP"
  local priv_var="BENCH_$((idx - 1))_IP"
  local pub="${!pub_var:-}"
  local priv="${!priv_var:-}"
  if [[ -n "${CHRONON_SSH_USE_PRIVATE:-}" ]]; then
    echo "$priv"
  elif [[ -n "$pub" && "$pub" != "None" ]]; then
    echo "$pub"
  else
    echo "$priv"
  fi
}

resolve_worker_ip() {
  local idx="$1"
  local var="WORKER_$((idx - 1))_IP"
  local ip
  ip="${!var:-}"
  if [[ -z "$ip" ]]; then
    echo "missing $var in instances.env" >&2
    return 1
  fi
  echo "$ip"
}

resolve_data_ssh_host() {
  if [[ -n "${DATA_PUBLIC_IP:-}" && "${DATA_PUBLIC_IP}" != "None" ]]; then
    echo "$DATA_PUBLIC_IP"
  else
    echo "${DATA_IP:-}"
  fi
}

bench_ssh_key() {
  require_ssh_key
  echo "${CHRONON_SSH_KEY}"
}

remote_bench_ssh_key() {
  echo "${REMOTE_BENCH_SSH_KEY}"
}
