//! Boot Chronon with Postgres + Redis composite storage (SQL durability, Redis claim queue).
//!
//! Requires PostgreSQL and Redis. Set `CHRONON_POSTGRES_URL` and ensure Redis is reachable
//! (default `redis://127.0.0.1:6379`, or `CHRONON_REDIS_URL`).
//!
//! ```bash
//! export CHRONON_POSTGRES_URL=postgres://user:pass@localhost/chronon
//! cargo run -p uf-chronon --example postgres_redis_boot --features postgres,redis
//! ```

use std::sync::Arc;

use chronon::prelude::*;
use chronon_backend_postgres::{postgres_test_url, PostgresSchedulerStore};
use chronon_backend_redis::{PostgresRedisSchedulerStore, RedisQueueLayer};

#[tokio::main]
async fn main() -> chronon::Result<()> {
    let pg_url = postgres_test_url();
    let redis_url =
        std::env::var("CHRONON_REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".into());

    let sql: Arc<dyn SchedulerStore> = Arc::new(PostgresSchedulerStore::connect(&pg_url).await?);
    let redis = RedisQueueLayer::connect(&redis_url, None).await?;
    let store: Arc<dyn SchedulerStore> = Arc::new(PostgresRedisSchedulerStore::new(sql, redis));

    let chronon = ChrononBuilder::new()
        .scheduler_store(store)
        .embedded()
        .build()?;

    assert_eq!(chronon.executor().script_count(), 0);
    eprintln!("Chronon booted with Postgres + Redis composite ({pg_url}, {redis_url})");
    Ok(())
}
