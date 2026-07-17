//! Redis ready-queue composite over a SQL [`SchedulerStore`](chronon_core::store::SchedulerStore).
//!
//! **Audience:** backend engineers deploying **Postgres + Redis** (SQL durability with a Redis
//! claim queue) for high worker claim throughput.
//!
//! ## Stack position
//!
//! ```text
//! chronon (facade, `postgres` + `redis` features) → chronon-backend-redis → chronon-backend-{postgres,sql-common} → chronon-core
//! ```
//!
//! PostgreSQL (or SQLite in tests) holds durable admin/history state; Redis sorted sets
//! (`{prefix}:ready:{pool}`) provide fast, ordered run claims.
//!
//! ## Entry points
//!
//! - [`RedisQueueLayer`] — ZADD / ZPOPMIN queue primitives
//! - [`PostgresRedisSchedulerStore`] — composite store delegating to SQL + Redis
//!
//! ## Prerequisites
//!
//! - Enable **`postgres` and `redis`** on the `chronon` facade (or depend on both adapter crates).
//! - Set `CHRONON_REDIS_URL` in production (or `CHRONON_TEST_REDIS_URL` in tests).
//! - Pass an optional Redis **key prefix** to isolate tenants (`connect(..., Some("myapp"))`).
//!
//! ## Example
//!
//! ```rust,no_run
//! use std::sync::Arc;
//!
//! use chronon_backend_postgres::PostgresSchedulerStore;
//! use chronon_backend_redis::{PostgresRedisSchedulerStore, RedisQueueLayer};
//! use chronon_core::store::SchedulerStore;
//!
//! # async fn example() -> chronon_core::Result<()> {
//! let sql: Arc<dyn SchedulerStore> = Arc::new(
//!     PostgresSchedulerStore::connect("postgres://localhost/chronon").await?,
//! );
//! let redis = RedisQueueLayer::connect("redis://127.0.0.1:6379", None).await?;
//! let store = PostgresRedisSchedulerStore::new(sql, redis);
//! # Ok(())
//! # }
//! ```

mod composite;
mod queue;

pub use composite::PostgresRedisSchedulerStore;
pub use queue::RedisQueueLayer;
