# Documentation verification baseline

Re-run after doc or test-harness changes. See [CONTRIBUTING.md](../CONTRIBUTING.md#documentation).

## Commands

```bash
export CARGO_BUILD_JOBS=1

# Workspace checks
cargo check -p uf-chronon --no-default-features
cargo check -p uf-chronon --features mem,telemetry-console,axum
cargo deny check
cargo clippy --workspace --all-targets -- -D warnings

# Unit + integration (exclude e2e/bench drivers)
cargo test --workspace --exclude chronon-e2e --exclude chronon-bench

# Matrix correctness (sequential — avoids shared bootstrap interference)
cargo test -p chronon-e2e -p chronon-axum -- --test-threads=1

# Store port contract
cargo test -p chronon-backend-mem --tests
cargo test -p chronon-backend-sqlite --tests

# Rustdoc tests (crates with # Examples)
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

# Facade examples
cargo run -p uf-chronon --example script_macro --features mem
cargo run -p uf-chronon --example script_handle_job --features mem
cargo run -p uf-chronon --example run_now --features mem
cargo run -p uf-chronon --example embedded_tick --features mem
cargo run -p uf-chronon --example store_router_boot --features mem
cargo run -p uf-chronon --example sqlite_boot --features sqlite
cargo run -p uf-chronon --example postgres_boot --features postgres
cargo run -p uf-chronon --example postgres_redis_boot --features postgres,redis
cargo run -p uf-chronon --example axum_host --features mem,axum

# Bench smoke
cargo run -p chronon-bench -- run \
  --experiment bm-ch0 --storage mem --deployment embedded \
  --telemetry off --ops 50 --warmup 5
```

## Facade examples

| Example | Features | Command |
|---------|----------|---------|
| `script_macro` | `mem` | `cargo run -p uf-chronon --example script_macro --features mem` |
| `script_handle_job` | `mem` | `cargo run -p uf-chronon --example script_handle_job --features mem` |
| `run_now` | `mem` | `cargo run -p uf-chronon --example run_now --features mem` |
| `embedded_tick` | `mem` | `cargo run -p uf-chronon --example embedded_tick --features mem` |
| `store_router_boot` | `mem` | `cargo run -p uf-chronon --example store_router_boot --features mem` |
| `sqlite_boot` | `sqlite` | `cargo run -p uf-chronon --example sqlite_boot --features sqlite` |
| `postgres_boot` | `postgres` | `cargo run -p uf-chronon --example postgres_boot --features postgres` |
| `postgres_redis_boot` | `postgres`, `redis` | `cargo run -p uf-chronon --example postgres_redis_boot --features postgres,redis` |
| `axum_host` | `mem`, `axum` | `cargo run -p uf-chronon --example axum_host --features mem,axum` |

## Baseline results (2026-07-08 quality pass)

| Check | Result |
|-------|--------|
| `cargo test --workspace --exclude chronon-e2e --exclude chronon-bench` | Run after changes |
| `cargo test -p chronon-e2e -p chronon-axum -- --test-threads=1` | 52 active (26 mem + 26 sqlite embedded/coordinator-worker); ignored postgres/postgres-redis run in PR `e2e-durable` |
| `cargo test -p chronon-backend-mem --tests` | store contract + global router smoke |
| `cargo test -p chronon-backend-sqlite --tests` | in-memory + file store contract (PR CI) |
| `chronon-scheduler` leader integration | leader module + store election |
| All facade examples (see table above) | Run after changes |
| BM-CH0 bench smoke | Run after changes |

## Line coverage (CI artifact)

PR CI runs a non-blocking [`coverage`](../.github/workflows/ci.yml) job with `cargo-llvm-cov`:

```bash
# Install once
cargo install cargo-llvm-cov --locked

# Summary to stdout (CI scope — excludes e2e/bench)
./scripts/coverage.sh --summary-only

# Full workspace including e2e
./scripts/coverage.sh --full --summary-only

# LCOV for local inspection
./scripts/coverage.sh --lcov --output-path lcov.info
```

**Baseline (2026-07-08):** ~55–60% line coverage on the CI-scoped slice; ~72% with full workspace including e2e.

Download `coverage-lcov` from the GitHub Actions run artifacts for the CI report.

## Coverage notes

- Behavioral coverage matrix: [`chronon-e2e/README.md`](../chronon-e2e/README.md)
- Shared store contract: [`chronon-testkit/src/store_contract.rs`](../chronon-testkit/src/store_contract.rs)
- Scenario catalog: [`chronon-testkit/src/catalog.rs`](../chronon-testkit/src/catalog.rs)
- Trait `# Contract` sections on [`SchedulerStore`](../chronon-core/src/store.rs)
