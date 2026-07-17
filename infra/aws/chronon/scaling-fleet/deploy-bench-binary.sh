#!/usr/bin/env bash
# Rsync source to bench_0, remote release build, fan-out binary to all bench hosts.
set -euo pipefail

SF="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SF/../../../.." && pwd)"
# shellcheck disable=SC1091
source "$SF/lib/bench-fleet.sh"

SSH_KEY="$(bench_ssh_key)"
SSH_OPTS=(-o StrictHostKeyChecking=no -o ConnectTimeout=30 -i "$SSH_KEY")
BENCH_COUNT="${BENCH_COUNT:-4}"
BENCH_BIN="${CHRONON_BENCH_BIN:-$HOME/chronon-bench/bin/chronon-bench}"
# Private git deps (e.g. quark) need gh credentials on the laptop — prefer a local release binary.
LOCAL_BIN="${CHRONON_BENCH_LOCAL_BIN:-}"

deploy_local_bin() {
  local bin="$1"
  local i host
  echo "deploying local binary ${bin} to ${BENCH_COUNT} bench host(s)..."
  for i in $(seq 1 "$BENCH_COUNT"); do
    host="$(resolve_bench_ip "$i")"
    echo "  host $i (${host})"
    ssh "${SSH_OPTS[@]}" "ec2-user@${host}" "mkdir -p ~/chronon-bench/bin ~/chronon/profiling/chronon-bench/reports ~/chronon/scaling-fleet/scripts"
    scp "${SSH_OPTS[@]}" "$bin" "ec2-user@${host}:~/chronon-bench/bin/chronon-bench"
    ssh "${SSH_OPTS[@]}" "ec2-user@${host}" "chmod +x ~/chronon-bench/bin/chronon-bench"
  done
  # Scripts still need the repo tree on bench_0 for fleet-local runners.
  local bench0
  bench0="$(resolve_bench_ip 1)"
  echo "rsync source (no build) to bench_0 (${bench0})..."
  rsync -az \
    --exclude 'target' --exclude 'target-*' --exclude '.git' --exclude 'profiling' \
    --exclude 'node_modules' --exclude '.cursor' \
    -e "ssh ${SSH_OPTS[*]}" \
    "$ROOT/" "ec2-user@${bench0}:~/chronon/"
}

if [[ -n "$LOCAL_BIN" ]]; then
  if [[ ! -x "$LOCAL_BIN" ]]; then
    echo "CHRONON_BENCH_LOCAL_BIN not executable: ${LOCAL_BIN}" >&2
    exit 1
  fi
  deploy_local_bin "$LOCAL_BIN"
  echo "deployed chronon-bench to $BENCH_COUNT bench host(s) (local binary)"
  exit 0
fi

bench0="$(resolve_bench_ip 1)"
echo "rsync source to bench_0 (${bench0})..."
rsync -az \
  --exclude 'target' --exclude 'target-*' --exclude '.git' --exclude 'profiling' \
  --exclude 'node_modules' --exclude '.cursor' \
  -e "ssh ${SSH_OPTS[*]}" \
  "$ROOT/" "ec2-user@${bench0}:~/chronon/"

# Private workspace deps (quark) — inject short-lived token from laptop `gh auth`.
GH_TOKEN="${CHRONON_GH_TOKEN:-}"
if [[ -z "$GH_TOKEN" ]] && command -v gh >/dev/null 2>&1; then
  GH_TOKEN="$(gh auth token 2>/dev/null || true)"
fi
if [[ -z "$GH_TOKEN" ]]; then
  echo "WARN: no GH token; remote cargo may fail on private git deps (set CHRONON_GH_TOKEN or gh auth login)" >&2
fi

echo "remote release build on bench_0..."
# Pass token as env on the remote command line (quoted); heredoc is stdin to remote bash.
ssh "${SSH_OPTS[@]}" "ec2-user@${bench0}" \
  "export GH_TOKEN=$(printf '%q' "$GH_TOKEN"); bash -s" <<'REMOTE'
set -euo pipefail
source ~/.cargo/env
export CARGO_BUILD_JOBS=4
export CARGO_TARGET_DIR=~/chronon/target-chronon-bench-release
export CARGO_NET_GIT_FETCH_WITH_CLI=true
if [[ -n "${GH_TOKEN:-}" ]]; then
  git config --global url."https://x-access-token:${GH_TOKEN}@github.com/".insteadOf "https://github.com/"
  cleanup_git_auth() {
    git config --global --unset-all "url.https://x-access-token:${GH_TOKEN}@github.com/.insteadof" 2>/dev/null || true
  }
  trap cleanup_git_auth EXIT
fi
cd ~/chronon
cargo build -p chronon-bench --release
mkdir -p ~/chronon-bench/bin ~/chronon/profiling/chronon-bench/reports
cp ~/chronon/target-chronon-bench-release/release/chronon-bench ~/chronon-bench/bin/chronon-bench
rm -rf ~/chronon/target ~/chronon/target/debug 2>/dev/null || true
REMOTE

for i in $(seq 2 "$BENCH_COUNT"); do
  host="$(resolve_bench_ip "$i")"
  echo "deploying binary to bench host $i (${host})..."
  ssh "${SSH_OPTS[@]}" "ec2-user@${host}" "mkdir -p ~/chronon-bench/bin ~/chronon/profiling/chronon-bench/reports"
  scp "${SSH_OPTS[@]}" \
    "ec2-user@${bench0}:~/chronon-bench/bin/chronon-bench" \
    "ec2-user@${host}:~/chronon-bench/bin/chronon-bench"
done

echo "deployed chronon-bench to $BENCH_COUNT bench host(s)"
