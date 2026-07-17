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

Runnable example: `cargo run -p uf-chronon --example postgres_redis_boot --features postgres,redis`.

## Configuration

| Option | Default | Purpose |
|--------|---------|---------|
| Redis URL | — | Pass to `RedisQueueLayer::connect` |
| `key_prefix` | `"chronon"` | Prefix for all keys (`{prefix}:ready:{pool}`); set per tenant when sharing Redis |
| `CHRONON_REDIS_URL` | — | Production URL (convention) |
| `CHRONON_TEST_REDIS_URL` | `redis://127.0.0.1:6379` | Test default via `RedisQueueLayer::test_url()` |

## Facade features

Enable **both** `postgres` and `redis`:

```toml
chronon = { package = "uf-chronon", version = "0.1", default-features = false, features = ["postgres", "redis"] }
```

The `redis` feature implies `postgres` in the facade manifest.

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
