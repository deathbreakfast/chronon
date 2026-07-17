#!/usr/bin/env bash
# Deploy pre-built test artifacts + source to e2e-runner and execute E2E gate.
#
# Required env:
#   CHRONON_E2E_HOST  — public/DNS hostname of the e2e runner
#   CHRONON_DATA_IP   — private IP of the data services host (Postgres/Redis)
#   CHRONON_SSH_KEY   — path to the SSH private key
set -euo pipefail

E2E_HOST="${CHRONON_E2E_HOST:?set CHRONON_E2E_HOST to the e2e-runner host}"
DATA_IP="${CHRONON_DATA_IP:?set CHRONON_DATA_IP to the data-services private IP}"
SSH_KEY="${CHRONON_SSH_KEY:?set CHRONON_SSH_KEY to the SSH private key path}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"

if [[ ! -f "$SSH_KEY" ]]; then
  echo "missing SSH key: $SSH_KEY" >&2
  exit 1
fi

SSH_OPTS=(-o StrictHostKeyChecking=no -o ConnectTimeout=30 -i "$SSH_KEY")

echo "== Local build (test artifacts) =="
cd "$ROOT"
export CARGO_BUILD_JOBS=1
cargo test -p chronon-e2e -p chronon-axum -p chronon-testkit -p chronon-backend-postgres -p chronon-backend-redis -p chronon-scheduler --no-run

echo "== Rsync repo + cargo git cache (no target) =="
rsync -az \
  --exclude target --exclude 'target-*' --exclude .git --exclude profiling \
  --exclude node_modules --exclude .cursor \
  -e "ssh ${SSH_OPTS[*]}" \
  "$ROOT/" "ec2-user@${E2E_HOST}:~/chronon/"

rsync -az -e "ssh ${SSH_OPTS[*]}" \
  "$HOME/.cargo/git/" "ec2-user@${E2E_HOST}:~/.cargo/git/" || true

ssh "${SSH_OPTS[@]}" "ec2-user@${E2E_HOST}" bash -s <<EOF
set -euo pipefail
export CARGO_BUILD_JOBS=1
export CARGO_INCREMENTAL=0
export RUST_BACKTRACE=1
export CHRONON_POSTGRES_URL=postgres://chronon:chronon@${DATA_IP}:5432/chronon
export CHRONON_REDIS_URL=redis://${DATA_IP}:6379
source \$HOME/.cargo/env
cd ~/chronon
./infra/aws/chronon/run-e2e-aws.sh
EOF
