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
cargo fmt --all -- --check
cargo machete
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --exclude uf-chronon --no-deps
RUSTDOCFLAGS="-D warnings" cargo doc -p uf-chronon --all-features --no-deps

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

# Public crate examples
cargo run -p uf-chronon --example script_macro --features mem
cargo run -p uf-chronon --example script_handle_job --features mem
cargo run -p uf-chronon --example run_now --features mem
cargo run -p uf-chronon --example embedded_tick --features mem
cargo run -p uf-chronon --example store_router_boot --features mem
cargo run -p uf-chronon --example sqlite_boot --features sqlite
cargo run -p uf-chronon --example postgres_boot --features postgres
cargo run -p uf-chronon --example postgres_redis_boot --features postgres,redis
cargo run -p uf-chronon --example axum_host --features mem,axum
cargo run -p uf-chronon --example axum_auth_wrap --features mem,axum
cargo run -p uf-chronon --example remote_http_client --features mem,axum
# Build-only for long-running split daemons (do not leave running in CI)
cargo build -p uf-chronon --example sqlite_coordinator_daemon --features sqlite
cargo build -p uf-chronon --example sqlite_worker_daemon --features sqlite
cargo build -p uf-chronon --example postgres_coordinator_daemon --features postgres
cargo build -p uf-chronon --example postgres_worker_daemon --features postgres
cargo build -p uf-chronon --example coordinator_daemon --features postgres,redis
cargo build -p uf-chronon --example worker_daemon --features postgres,redis

# Bench smoke
cargo run -p chronon-bench -- run \
  --experiment bm-ch0 --storage mem --deployment embedded \
  --telemetry off --ops 50 --warmup 5
```

## Public crate examples

Canonical path (multi-worker recipes: [`chronon/README.md`](../chronon/README.md#how-to-run-examples)):

| Example | Topology | Features | Command |
|---------|----------|----------|---------|
| `sqlite_boot` | Embedded | `sqlite` | `cargo run -p uf-chronon --example sqlite_boot --features sqlite` |
| `sqlite_coordinator_daemon` / `sqlite_worker_daemon` | Coordinator–worker (local) | `sqlite` | see chronon README runbook |
| `coordinator_daemon` / `worker_daemon` | Coordinator–worker (Postgres+Redis) | `postgres`, `redis` | see chronon README runbook |
| `remote_http_client` | Remote HTTP client | `mem`, `axum` | `cargo run -p uf-chronon --example remote_http_client --features mem,axum` |

Other examples: `script_macro`, `script_handle_job`, `run_now`, `embedded_tick`, `store_router_boot`, `postgres_boot`, `postgres_redis_boot`, `axum_host`, `axum_auth_wrap`, `postgres_coordinator_daemon`, `postgres_worker_daemon`.

## Security hardening checks (axum / runtime / sql-common)

After changes to AdminAuth, actor policy, error sanitize/redact, HTTP upsert, actor snapshot, list bounds, revision redaction, or schema allowlisting:

```bash
cargo test -p chronon-axum --test router_smoke -- --test-threads=1
cargo test -p chronon-core --lib
cargo test -p chronon-executor --lib
cargo test -p chronon-runtime --lib
cargo test -p chronon-backend-sql-common --lib
cargo run -p uf-chronon --example axum_auth_wrap --features mem,axum
```

These are **correctness** gates (happy + sad paths). They are not BM-CH benchmark subjects; see [`chronon-bench/EXPERIMENTS.md`](../chronon-bench/EXPERIMENTS.md).

**AWS e2e:** No new fleet topology. Waive new AWS scenarios — AdminAuth / actor policy / sanitize are covered by `chronon-axum` + unit tests on every PR; existing catalog (`actor_snapshot_toctou`) remains sufficient.

**Bench:** Correctness-only waiver — no hot-path change requiring BM-CH re-measure.

## Baseline results (2026-07-08 quality pass)

| Check | Result |
|-------|--------|
| `cargo test --workspace --exclude chronon-e2e --exclude chronon-bench` | Run after changes |
| `cargo test -p chronon-e2e -p chronon-axum -- --test-threads=1` | 56 active (28 mem + 28 sqlite embedded/coordinator-worker; 14 catalog scenarios × 2 deployments × 2 backends); ignored postgres/postgres-redis run in PR `e2e-durable` |
| `cargo test -p chronon-backend-mem --tests` | store contract + global router smoke |
| `cargo test -p chronon-backend-sqlite --tests` | in-memory + file store contract (PR CI) |
| `chronon-scheduler` leader integration | leader module + store election |
| All public crate examples (see table above) | Run after changes |
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

## Test Map (security hardening)

| ID | Behavior | Primary tests | Notes |
|----|----------|---------------|-------|
| C-1 | Require flag without verifier | `router_smoke::builder_requires_admin_auth_when_flagged`, `chronon_axum::tests::builder_requires_auth_when_flagged` | Sad: `build()` Err |
| C-1 | Token auth 401 / 200 | `admin_auth_missing_token_401`, `admin_auth_valid_token_200` | Header `x-chronon-admin-token` |
| C-1 | Host middleware wrap | `auth_middleware_rejects_without_bearer` + `axum_auth_wrap` | Happy `200` / sad `401` |
| C-2 | System actor rejected on HTTP | `upsert_rejects_system_shaped_actor_json`; core `actor_policy::tests::reject_external_system` | Sad `400`; internal OK |
| C-3 | URL userinfo redaction | core `redact::tests::*`; sql-common `map_connect_err_redacts_userinfo` | Connect + text scan |
| C-4 | Run error sanitize | core `sanitize::tests::*`; runtime `sanitize_strips_secret_prefixes_before_persist` | Persist + HTTP envelope |
| — | Upsert-by-name | `upsert_job_preserves_job_id_by_name` | Happy preserve `job_id` + revision bump |
| — | Upsert script mismatch / missing | `upsert_job_script_not_found` | Sad `400` with error body |
| — | Policy clamps | `upsert_job_clamps_extreme_policy_knobs`, core `clamp_security_bounds` | Concurrent / timeout / retry caps |
| — | List limit clamp | `handlers::tests::clamp_list_limit_*`, `list_runs_clamps_limit` | Cap = `MAX_LIST_LIMIT` |
| — | Actor snapshot (TOCTOU) | executor `spawn_run_uses_run_actor_json_not_live_job`; catalog `actor_snapshot_toctou` | Run snapshot wins |
| — | Schema allowlist | `schema_name_accepts_*` / `schema_name_rejects_*` | Isolated connect → `ParamError` |
| — | Revision redaction | `get_job_revisions_redacts_sensitive_fields` | HTTP nulls; store keeps full |
| — | Event transitions | `chronon-runtime` `events::tests` | Illegal transitions ignored |

## Documentation Map (hardening surface)

| Topic | Landing | Mid | Deep |
|-------|---------|-----|------|
| AdminAuth / RequireAdmin | `uf-chronon` Features + `chronon-axum` crate root | `SECURITY.md` operator table | `axum_auth_wrap` example |
| External actor policy | `SECURITY.md` HTTP `actor_json` | `RejectExternalSystemActor` | upsert handler |
| Upsert-by-name / bounds / redaction / schema | `uf-chronon` Features + Architecture | `chronon-axum` handlers / `SECURITY.md` | sql-common `validate_postgres_schema_name` |
| Actor snapshot at execute | `uf-chronon` Host identity Feature | `ContextFactory` / executor `spawn_run` | catalog `actor_snapshot_toctou` |
| Error sanitize / URL redact | `SECURITY.md` Errors | `sanitize_error_message` / `redact_endpoint` | connect `map_connect_err` |
| Body size | `SECURITY.md` (`DefaultBodyLimit`) | — | host Axum config |
| Verification gates | this file + `CONTRIBUTING.md` | rustdoc + cargo checklist | e2e / AWS scripts |
