# chronon (public crate)

Public crate re-exporting split upstream crates.

## Cargo features

```toml
# crates.io package is uf-chronon; Rust import stays `use chronon::…`.
chronon = { package = "uf-chronon", version = "0.1", default-features = false, features = ["mem", "axum"] }
```

| Feature | Forwards to | Notes |
|---------|-------------|-------|
| `mem` | `chronon-backend-mem` | In-process store for dev and tests |
| `sqlite` | `chronon-backend-sqlite` | File or `:memory:` SQLite |
| `postgres` | `chronon-backend-postgres` | Shared PostgreSQL pool |
| `redis` | `chronon-backend-redis` | SQL durability + Redis claim queue — **enable `postgres` too** |
| `telemetry-console` | Documents console sink usage (always available via `ConsoleSink`) |
| `axum` | `chronon-axum` router and state types |

## Prelude

```rust
use chronon::prelude::*;
```

## Configuration

Settings merge in this order (explicit builder values win over environment defaults):

| Setting | Builder API | Environment variable | Default |
|---------|-------------|---------------------|---------|
| Scheduler store | `.scheduler_store()` / `.scheduler_store_from_global()` | — | required |
| Context factory | `.context_factory()` | — | `NoOpContextFactory` |
| Telemetry | `.telemetry_sink()` | — | `NoOpSink` |
| Script registry | `.script_registry()` / `.auto_registry()` | — | empty or inventory |
| Tick interval | `.tick_interval_ms()` | `CHRONON_TICK_INTERVAL_MS` | 250 ms |
| Instance id | `.instance_id()` | — | random UUID |
| Partition count | — (env only) | `CHRONON_NUM_PARTITIONS` | 64 |
| Tick batch limit | — | `CHRONON_TICK_BATCH_LIMIT` | 500 |
| Worker pool | — | `CHRONON_WORKER_POOL` | `"general"` |
| Worker concurrency | — | `CHRONON_WORKER_CONCURRENCY` | 4 |

### Backend connection (not on `ChrononBuilder`)

| Backend | How to configure |
|---------|------------------|
| PostgreSQL | Connection URL to `PostgresSchedulerStore::connect` or `CHRONON_POSTGRES_URL` / `CHRONON_TEST_POSTGRES_URL` for tests |
| SQLite | File path (`SqliteSchedulerStore::new`) or URL (`connect`, including `:memory:`) |
| Redis overlay | URL to `RedisQueueLayer::connect`; optional `key_prefix` (default `chronon`); `CHRONON_REDIS_URL` / `CHRONON_TEST_REDIS_URL` in tests |

Builder `.tick_interval_ms()` overrides `CHRONON_TICK_INTERVAL_MS`. Partition count and lease TTLs are read from the environment only — see `chronon-scheduler` rustdoc for the full env-var table.

## How to run examples

Canonical teaching path (start here). Topology docs:
[Embedded](https://docs.rs/uf-chronon/latest/chronon/index.html#embedded-one-process) /
[Coordinator–worker](https://docs.rs/uf-chronon/latest/chronon/index.html#coordinator-worker-split) /
[Remote HTTP client](https://docs.rs/uf-chronon/latest/chronon/index.html#remote-http-client).

### 1. Embedded — `sqlite_boot` (standalone)

One process, file-backed store. No external services.

```bash
cargo run -p uf-chronon --example sqlite_boot --features sqlite
# optional: CHRONON_SQLITE_PATH=:memory:  or a custom file path
# default path: /tmp/chronon-example.db
```

Success: stderr prints `Chronon booted with SQLite store (…)`.

### 2. Coordinator–worker (multi-process — run as a set)

Coordinator and workers share one store. They are **not** useful alone.

| Rule | Detail |
|------|--------|
| Shared env | Same `CHRONON_SQLITE_PATH` or same Postgres/Redis URLs on every process |
| Start order | Coordinator first (`init_partitions`), then workers |
| Workers | Each needs a unique `CHRONON_INSTANCE_ID`; optional `CHRONON_WORKER_POOL` (default `general`) |
| Scripts | Execute on **workers** only; example daemons register `daemon-noop` |

**Local SQLite** (no Postgres/Redis) — 1 coordinator + 2 workers:

```bash
export CHRONON_SQLITE_PATH=/tmp/chronon-split.db

# Terminal 1 — coordinator
cargo run -p uf-chronon --example sqlite_coordinator_daemon --features sqlite

# Terminal 2 — worker A
CHRONON_INSTANCE_ID=worker-a cargo run -p uf-chronon --example sqlite_worker_daemon --features sqlite

# Terminal 3 — worker B
CHRONON_INSTANCE_ID=worker-b cargo run -p uf-chronon --example sqlite_worker_daemon --features sqlite
```

**Production claim path (Postgres + Redis)** — same pattern:

```bash
export CHRONON_POSTGRES_URL=postgres://user:pass@localhost/chronon
export CHRONON_REDIS_URL=redis://127.0.0.1:6379

# Terminal 1 — coordinator
cargo run -p uf-chronon --example coordinator_daemon --features postgres,redis

# Terminal 2 / 3 — workers (unique instance ids)
CHRONON_INSTANCE_ID=worker-a cargo run -p uf-chronon --example worker_daemon --features postgres,redis
CHRONON_INSTANCE_ID=worker-b cargo run -p uf-chronon --example worker_daemon --features postgres,redis
```

Stop with Ctrl-C on each process. Real apps link `#[chronon::script]` into worker binaries and use `.auto_registry()`.

### 3. Remote HTTP client — `remote_http_client` (standalone)

App schedules via `RemoteCoordinatorClient` (no local Chronon loops). This demo spins a short-lived mem API host, then upserts + `run_now`.

```bash
cargo run -p uf-chronon --example remote_http_client --features mem,axum
```

**Production:** Chronon does not authenticate `/api/chronon/*`. Wrap with host auth — see `axum_auth_wrap` and repository [`SECURITY.md`](../SECURITY.md).

### Other examples

| Example | Topology | Features | Notes |
|---------|----------|----------|-------|
| `script_macro`, `script_handle_job`, `run_now`, `embedded_tick` | Embedded | `mem` | API / scheduling demos |
| `store_router_boot` | Embedded | `mem` | Global store router |
| `postgres_boot`, `postgres_redis_boot` | Embedded | `postgres` / `postgres,redis` | Store wiring |
| `axum_host`, `axum_auth_wrap` | Embedded + HTTP | `mem,axum` | Router / Bearer demo |
| `postgres_coordinator_daemon`, `postgres_worker_daemon` | Coordinator–worker | `postgres` | Postgres-only split |

## Documentation

API reference: `cargo doc -p uf-chronon --all-features --open`. See root [`README.md`](../README.md) for architecture.
