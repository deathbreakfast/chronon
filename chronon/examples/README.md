# Chronon examples

Runnable proofs for embedded scheduling, coordinator–worker splits, and remote HTTP clients. The canonical path below matches the crate README; use secondary examples when you need a specific backend, HTTP mount, or API surface.

Full multi-terminal runbooks: [`../README.md` — How to run examples](../README.md#how-to-run-examples).

## Canonical path

### 1. Embedded — [`sqlite_boot.rs`](sqlite_boot.rs)

One process, file-backed store — proves builder + SQLite adapter without external services.

```bash
cargo run -p uf-chronon --example sqlite_boot --features sqlite
```

Success: `Chronon booted with SQLite store (…)`.

### 2. Coordinator–worker — [`sqlite_coordinator_daemon.rs`](sqlite_coordinator_daemon.rs) · [`sqlite_worker_daemon.rs`](sqlite_worker_daemon.rs)

Shared `CHRONON_SQLITE_PATH`, coordinator first, unique `CHRONON_INSTANCE_ID` per worker — models production split on the cheapest local backend.

```bash
export CHRONON_SQLITE_PATH=/tmp/chronon-split.db
cargo run -p uf-chronon --example sqlite_coordinator_daemon --features sqlite
# CHRONON_INSTANCE_ID=worker-a cargo run -p uf-chronon --example sqlite_worker_daemon --features sqlite
```

Success: `sqlite_coordinator_daemon: running (…)` and `sqlite_worker_daemon: … pool=…`.

Production-shaped variant: `coordinator_daemon` + `worker_daemon` with `postgres,redis` — same pattern, different URLs (see crate README).

### 3. Remote HTTP client — [`remote_http_client.rs`](remote_http_client.rs)

App schedules through `RemoteCoordinatorClient` with no local tick loop — useful when only the coordinator host runs Chronon loops.

```bash
cargo run -p uf-chronon --example remote_http_client --features mem,axum
```

Success: `RemoteCoordinatorClient upsert + run_now ok — run_id=…`.

## Other examples

| Example | When you'd open it | Command | Success signal |
|---------|-------------------|---------|----------------|
| [`script_macro.rs`](script_macro.rs) | `#[chronon::script]` + tick enqueue | `cargo run -p uf-chronon --example script_macro --features mem` | `script registered; tick enqueued … run(s)` |
| [`script_handle_job.rs`](script_handle_job.rs) | Build a `Job` handle without macro sugar | `cargo run -p uf-chronon --example script_handle_job --features mem` | `handle-built job; tick enqueued … run(s)` |
| [`run_now.rs`](run_now.rs) | Manual immediate run of a cron job | `cargo run -p uf-chronon --example run_now --features mem` | `run_now enqueued run_id=…` |
| [`embedded_tick.rs`](embedded_tick.rs) | Tick loop enqueue without script registry noise | `cargo run -p uf-chronon --example embedded_tick --features mem` | `tick enqueued … run(s)` |
| [`store_router_boot.rs`](store_router_boot.rs) | Global mem store router wiring | `cargo run -p uf-chronon --example store_router_boot --features mem` | `Chronon booted from global mem store` |
| [`postgres_boot.rs`](postgres_boot.rs) | Postgres store in one process | `cargo run -p uf-chronon --example postgres_boot --features postgres` | `Chronon booted with PostgreSQL store (…)` |
| [`postgres_redis_boot.rs`](postgres_redis_boot.rs) | Postgres + Redis composite in one process | `cargo run -p uf-chronon --example postgres_redis_boot --features postgres,redis` | `Chronon booted with Postgres + Redis composite (…)` |
| [`axum_host.rs`](axum_host.rs) | Mount unauthenticated `/api/chronon` on Axum | `cargo run -p uf-chronon --example axum_host --features mem,axum` | `Chronon API mounted at … — listed 1 script` |
| [`axum_auth_wrap.rs`](axum_auth_wrap.rs) | Bearer token gate before Chronon routes | `cargo run -p uf-chronon --example axum_auth_wrap --features mem,axum` | `… missing token → 401, x-chronon-admin-token → listed 1 script` |
| [`postgres_coordinator_daemon.rs`](postgres_coordinator_daemon.rs) | Postgres-only coordinator split | `cargo run -p uf-chronon --example postgres_coordinator_daemon --features postgres` | `postgres_coordinator_daemon: running (…)` |
| [`postgres_worker_daemon.rs`](postgres_worker_daemon.rs) | Postgres-only worker split | `cargo run -p uf-chronon --example postgres_worker_daemon --features postgres` | `postgres_worker_daemon: … pool=…` |

Topology reference: [Embedded](https://docs.rs/uf-chronon/latest/chronon/index.html#embedded-one-process) · [Coordinator–worker](https://docs.rs/uf-chronon/latest/chronon/index.html#coordinator-worker-split) · [Remote HTTP client](https://docs.rs/uf-chronon/latest/chronon/index.html#remote-http-client).
