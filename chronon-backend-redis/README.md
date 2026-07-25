# chronon-backend-redis

Postgres + Redis composite: SQL durability with a Redis sorted-set claim queue.

## Audience

**Backend engineers** deploying high worker claim throughput — SQL holds admin/history; Redis orders `claim_next_queued`.

## Components

| Type | Role |
|------|------|
| [`RedisQueueLayer`] | ZADD / ZPOPMIN on `{prefix}:ready:{pool}` |
| [`PostgresRedisSchedulerStore`] | Wraps `Arc<dyn SchedulerStore>` + Redis; enqueues on `create_run`, claims via Redis |

## Compose with Chronon

```rust
use std::sync::Arc;
use chronon::prelude::*;
use chronon_backend_postgres::PostgresSchedulerStore;
use chronon_backend_redis::{PostgresRedisSchedulerStore, RedisQueueLayer};

let sql: Arc<dyn SchedulerStore> = Arc::new(
    PostgresSchedulerStore::connect("postgres://localhost/chronon").await?,
);
let redis = RedisQueueLayer::connect("redis://127.0.0.1:6379", None).await?;
let store: Arc<dyn SchedulerStore> = Arc::new(PostgresRedisSchedulerStore::new(sql, redis));
let chronon = ChrononBuilder::new()
    .scheduler_store(store)
    .embedded()
    .build()?;
```

Runnable examples (full runbook: [`chronon/README.md`](../chronon/README.md#how-to-run-examples)):

```bash
# Embedded boot
cargo run -p uf-chronon --example postgres_redis_boot --features postgres,redis

# Coordinator–worker — shared URLs; coordinator first; unique worker ids
export CHRONON_POSTGRES_URL=postgres://user:pass@localhost/chronon
export CHRONON_REDIS_URL=redis://127.0.0.1:6379
# Terminal 1
cargo run -p uf-chronon --example coordinator_daemon --features postgres,redis
# Terminal 2
CHRONON_INSTANCE_ID=worker-a cargo run -p uf-chronon --example worker_daemon --features postgres,redis
# Terminal 3
CHRONON_INSTANCE_ID=worker-b cargo run -p uf-chronon --example worker_daemon --features postgres,redis
```

Topology docs: [Embedded](https://docs.rs/uf-chronon/latest/chronon/index.html#embedded-one-process) /
[Coordinator–worker](https://docs.rs/uf-chronon/latest/chronon/index.html#coordinator-worker-split).

## Configuration

| Option | Default | Purpose |
|--------|---------|---------|
| Redis URL | — | Pass to `RedisQueueLayer::connect` |
| `key_prefix` | `"chronon"` | Prefix for all keys (`{prefix}:ready:{pool}`); set per tenant when sharing Redis |
| `CHRONON_REDIS_URL` | — | Production URL (convention) |
| `CHRONON_TEST_REDIS_URL` | `redis://127.0.0.1:6379` | Test default via `RedisQueueLayer::test_url()` |

## Cargo features

Enable **both** `postgres` and `redis`:

```toml
chronon = { package = "uf-chronon", version = "0.1", default-features = false, features = ["postgres", "redis"] }
```

The `redis` feature implies `postgres` in the public crate manifest.

## Contract tests

```bash
# SQLite + Redis (default, needs local Redis)
cargo test -p chronon-backend-redis --tests

# Postgres + Redis (ignored; tag CI)
export CHRONON_POSTGRES_URL=postgres://...
cargo test -p chronon-backend-redis --tests -- --include-ignored
```

## Documentation

```bash
cargo doc -p chronon-backend-redis --no-deps --open
```

[`RedisQueueLayer`]: https://docs.rs/chronon-backend-redis/latest/chronon_backend_redis/struct.RedisQueueLayer.html
[`PostgresRedisSchedulerStore`]: https://docs.rs/chronon-backend-redis/latest/chronon_backend_redis/struct.PostgresRedisSchedulerStore.html
