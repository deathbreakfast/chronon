#!/usr/bin/env bash
# Mirror full PR CI (ci.yml) on an AWS host with Rust toolchain.
#
# Modes:
#   CHRONON_REMOTE_CI_COMPOSE=1 (default) — start postgres/redis via
#     infra/aws/chronon/docker-compose.data.yml on the CI host (127.0.0.1).
#   CHRONON_REMOTE_CI_COMPOSE=0 — use CHRONON_DATA_IP for an external data node.
#
# Does not run distributed multi-process smokes — use run-e2e-aws.sh.
#
# Required env:
#   CHRONON_E2E_HOST — CI/e2e host
#   CHRONON_SSH_KEY  — path to the SSH private key
# Optional:
#   CHRONON_DATA_IP, CHRONON_REMOTE_CI_DIR, CHRONON_REMOTE_CI_COMPOSE
set -euo pipefail

E2E_HOST="${CHRONON_E2E_HOST:?set CHRONON_E2E_HOST to the remote CI host}"
SSH_KEY="${CHRONON_SSH_KEY:?set CHRONON_SSH_KEY to the SSH private key path}"
REMOTE_DIR="${CHRONON_REMOTE_CI_DIR:-/home/ec2-user/chronon-remote-ci}"
USE_LOCAL_COMPOSE="${CHRONON_REMOTE_CI_COMPOSE:-1}"
if [[ "$USE_LOCAL_COMPOSE" == "1" ]]; then
  DATA_IP="${CHRONON_DATA_IP:-127.0.0.1}"
else
  DATA_IP="${CHRONON_DATA_IP:?set CHRONON_DATA_IP when CHRONON_REMOTE_CI_COMPOSE=0}"
fi
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"

if [[ ! -f "$SSH_KEY" ]]; then
  echo "missing SSH key: $SSH_KEY" >&2
  exit 1
fi

SSH_OPTS=(-o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o ConnectTimeout=30 -i "$SSH_KEY")

echo ">>> rsync repo to $E2E_HOST:$REMOTE_DIR"
ssh "${SSH_OPTS[@]}" "ec2-user@${E2E_HOST}" "mkdir -p $REMOTE_DIR"
rsync -az --delete \
  --exclude target --exclude 'target-*' --exclude .git --exclude profiling \
  --exclude node_modules --exclude .cursor \
  -e "ssh ${SSH_OPTS[*]}" \
  "$ROOT/" "ec2-user@${E2E_HOST}:${REMOTE_DIR}/"

rsync -az -e "ssh ${SSH_OPTS[*]}" \
  "$HOME/.cargo/git/" "ec2-user@${E2E_HOST}:~/.cargo/git/" || true

echo ">>> run full PR CI mirror on $E2E_HOST (compose=$USE_LOCAL_COMPOSE data=$DATA_IP)"
ssh "${SSH_OPTS[@]}" "ec2-user@${E2E_HOST}" \
  "export CHRONON_REMOTE_CI_COMPOSE=$USE_LOCAL_COMPOSE CHRONON_DATA_IP=$DATA_IP CARGO_BUILD_JOBS=\${CARGO_BUILD_JOBS:-1}; \
   chmod +x $REMOTE_DIR/infra/aws/chronon/run-remote-ci-inner.sh; \
   $REMOTE_DIR/infra/aws/chronon/run-remote-ci-inner.sh"

echo "Remote CI passed on $E2E_HOST"
