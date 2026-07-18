#!/usr/bin/env bash
# Inner CI steps executed on the remote host (invoked by run-remote-ci.sh).
set -euo pipefail

# shellcheck disable=SC1091
source "$HOME/.cargo/env" 2>/dev/null || true
if ! command -v cargo >/dev/null 2>&1; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
  # shellcheck disable=SC1091
  source "$HOME/.cargo/env"
fi
rustup component add clippy 2>/dev/null || true

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$ROOT"

USE_LOCAL_COMPOSE="${CHRONON_REMOTE_CI_COMPOSE:-1}"
DATA_IP="${CHRONON_DATA_IP:-127.0.0.1}"

export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/chronon-remote-ci-target}"
export CARGO_INCREMENTAL=0
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-1}"
export RUST_BACKTRACE=1
# Durable URLs are set only for the e2e-durable section below so workspace
# unit tests do not unexpectedly bind to a shared redis/postgres instance.

mkdir -p "$CARGO_TARGET_DIR"

if [[ "$USE_LOCAL_COMPOSE" == "1" ]]; then
  echo "=== ensure docker + compose data services ==="
  if ! command -v docker >/dev/null 2>&1; then
    sudo dnf install -y docker || sudo yum install -y docker
    sudo systemctl enable --now docker
  fi
  sudo systemctl start docker
  if ! command -v docker-compose >/dev/null 2>&1; then
    sudo curl -fsSL "https://github.com/docker/compose/releases/download/v2.29.7/docker-compose-linux-x86_64" \
      -o /usr/local/bin/docker-compose
    sudo chmod +x /usr/local/bin/docker-compose
  fi
  sudo docker-compose -f infra/aws/chronon/docker-compose.data.yml up -d
  for _ in $(seq 1 60); do
    if sudo docker-compose -f infra/aws/chronon/docker-compose.data.yml exec -T postgres \
      pg_isready -U chronon >/dev/null 2>&1; then
      echo "postgres ready"
      break
    fi
    sleep 2
  done
fi

echo "=== check (chronon facade) ==="
cargo check -p uf-chronon --no-default-features
cargo check -p uf-chronon --features mem,telemetry-console
cargo check -p uf-chronon --features mem,telemetry-console,axum

echo "=== deny ==="
if ! command -v cargo-deny >/dev/null 2>&1; then
  cargo install cargo-deny --locked
fi
cargo deny check

echo "=== clippy (workspace) ==="
cargo clippy --workspace --all-targets -- -D warnings

echo "=== testkit ==="
cargo test -p chronon-testkit

echo "=== workspace (exclude e2e/bench) ==="
cargo test --workspace --exclude chronon-e2e --exclude chronon-bench

echo "=== e2e + axum (mem/sqlite) ==="
cargo test -p chronon-e2e -p chronon-axum -- --test-threads=1

echo "=== backend-stores mem/sqlite ==="
cargo test -p chronon-backend-mem --tests
cargo test -p chronon-backend-sqlite --tests

echo "=== scheduler integration ==="
cargo test -p chronon-scheduler --tests

echo "=== bench-smoke BM-CH0 ==="
cargo run -p chronon-bench -- run \
  --experiment bm-ch0 --storage mem --deployment embedded \
  --telemetry off --ops 50 --warmup 5

echo "=== examples ==="
cargo run -p uf-chronon --example script_macro --features mem
cargo run -p uf-chronon --example script_handle_job --features mem
cargo run -p uf-chronon --example run_now --features mem
cargo run -p uf-chronon --example embedded_tick --features mem
cargo run -p uf-chronon --example store_router_boot --features mem
cargo run -p uf-chronon --example sqlite_boot --features sqlite
cargo run -p uf-chronon --example axum_host --features mem,axum

echo "=== docs ==="
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --exclude uf-chronon --no-deps
RUSTDOCFLAGS="-D warnings" cargo doc -p uf-chronon --all-features --no-deps
cargo test --doc -p chronon-core
cargo test --doc -p chronon-backend-mem
cargo test --doc -p chronon-backend-sql-common
cargo test --doc -p chronon-backend-postgres
cargo test --doc -p chronon-backend-sqlite
cargo test --doc -p chronon-backend-redis
cargo test --doc -p chronon-runtime
cargo test --doc -p chronon-scheduler
cargo test --doc -p chronon-executor
cargo test --doc -p chronon-axum

echo "=== machete ==="
if ! command -v cargo-machete >/dev/null 2>&1; then
  cargo install cargo-machete --locked
fi
cargo machete

echo "=== e2e-durable (postgres/redis; distributed smokes excluded) ==="
export CHRONON_POSTGRES_URL="${CHRONON_POSTGRES_URL:-postgres://chronon:chronon@${DATA_IP}:5432/chronon}"
export CHRONON_REDIS_URL="${CHRONON_REDIS_URL:-redis://${DATA_IP}:6379}"
cargo test -p chronon-backend-postgres --tests -- --include-ignored
cargo test -p chronon-backend-redis --tests -- --include-ignored
cargo test -p chronon-scheduler --test leader_integration
# scenarios binary no longer embeds distributed suite (see distributed_smoke.rs).
cargo test -p chronon-e2e --test scenarios -- --ignored --test-threads=1
cargo run -p uf-chronon --example postgres_boot --features postgres
cargo run -p uf-chronon --example postgres_redis_boot --features postgres,redis

echo "Remote CI inner script passed (distributed smokes excluded)."
