# chronon (facade)

Public facade re-exporting split upstream crates.

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

## Examples

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

## Documentation

API reference: `cargo doc -p uf-chronon --all-features --open`. See root [`README.md`](../README.md) for architecture.
