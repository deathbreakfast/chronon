//! Redis ready-queue composite over a SQL [`SchedulerStore`](chronon_core::store::SchedulerStore).
//!
//! **When to use:** production **Mode 2** (and Mode 1) when you want Postgres durability with a
//! Redis claim hot path. Enable **`postgres` and `redis`** on the `chronon` facade.
//!
//! Getting started:
//! [Mode 1](https://docs.rs/uf-chronon/latest/chronon/index.html#mode-1--embedded-one-binary) /
//! [Mode 2](https://docs.rs/uf-chronon/latest/chronon/index.html#mode-2--coordinator--worker-two-binaries).
//!
//! ## Stack position
//!
//! ```text
//! chronon (facade, `postgres` + `redis` features) → chronon-backend-redis → chronon-backend-{postgres,sql-common} → chronon-core
//! ```
//!
//! PostgreSQL holds durable admin/history state; Redis sorted sets (`{prefix}:ready:{pool}`)
//! provide fast, ordered run claims.
//!
//! ## Entry points
//!
//! - [`RedisQueueLayer`] — ZADD / ZPOPMIN queue primitives
//! - [`PostgresRedisSchedulerStore`] — composite store delegating to SQL + Redis
//!
//! ## Prerequisites
//!
//! - Set `CHRONON_POSTGRES_URL` and `CHRONON_REDIS_URL` (or `CHRONON_TEST_*` in tests).
//! - Optional Redis **key prefix** to isolate tenants (`connect(..., Some("myapp"))`).
//!
//! ## Mode 1 — Embedded
//!
//! ```ignore
//! use std::sync::Arc;
//! use chronon::prelude::*;
//! use chronon::{PostgresRedisSchedulerStore, PostgresSchedulerStore, RedisQueueLayer};
//!
//! let pg = std::env::var("CHRONON_POSTGRES_URL")?;
//! let redis_url = std::env::var("CHRONON_REDIS_URL")?;
//! let sql: Arc<dyn SchedulerStore> =
//!     Arc::new(PostgresSchedulerStore::connect(&pg).await?);
//! let redis = RedisQueueLayer::connect(&redis_url, None).await?;
//! let store = PostgresRedisSchedulerStore::new(sql, redis);
//! let chronon = ChrononBuilder::new()
//!     .scheduler_store(Arc::new(store))
//!     .context_factory(Arc::new(JsonScriptContextFactory))
//!     .embedded()
//!     .auto_registry()
//!     .build()?;
//! ```
//!
//! Runnable: `cargo run -p uf-chronon --example postgres_redis_boot --features postgres,redis`
//!
//! ## Mode 2 — Coordinator binary
//!
//! Shared Postgres + Redis with workers. Tick only:
//!
//! ```ignore
//! use std::sync::Arc;
//! use chronon::prelude::*;
//! use chronon::{PostgresRedisSchedulerStore, PostgresSchedulerStore, RedisQueueLayer};
//!
//! let pg = std::env::var("CHRONON_POSTGRES_URL")?;
//! let redis_url = std::env::var("CHRONON_REDIS_URL")?;
//! let sql: Arc<dyn SchedulerStore> =
//!     Arc::new(PostgresSchedulerStore::connect(&pg).await?);
//! let redis = RedisQueueLayer::connect(&redis_url, None).await?;
//! let store = PostgresRedisSchedulerStore::new(sql, redis);
//! let mut chronon = ChrononBuilder::new()
//!     .scheduler_store(Arc::new(store))
//!     .context_factory(Arc::new(JsonScriptContextFactory))
//!     .instance_id("coordinator-0")
//!     .coordinator_only()
//!     .build()?;
//! chronon.scheduler.init_partitions().await;
//! chronon.run().await?;
//! ```
//!
//! Runnable: `cargo run -p uf-chronon --example coordinator_daemon --features postgres,redis`
//!
//! ## Mode 2 — Worker binary
//!
//! Same URLs, unique `CHRONON_INSTANCE_ID`, scripts via `.auto_registry()`:
//!
//! ```ignore
//! use std::sync::Arc;
//! use chronon::prelude::*;
//! use chronon::{PostgresRedisSchedulerStore, PostgresSchedulerStore, RedisQueueLayer};
//!
//! let pg = std::env::var("CHRONON_POSTGRES_URL")?;
//! let redis_url = std::env::var("CHRONON_REDIS_URL")?;
//! let sql: Arc<dyn SchedulerStore> =
//!     Arc::new(PostgresSchedulerStore::connect(&pg).await?);
//! let redis = RedisQueueLayer::connect(&redis_url, None).await?;
//! let store = PostgresRedisSchedulerStore::new(sql, redis);
//! let mut chronon = ChrononBuilder::new()
//!     .scheduler_store(Arc::new(store))
//!     .context_factory(Arc::new(JsonScriptContextFactory))
//!     .instance_id(std::env::var("CHRONON_INSTANCE_ID").unwrap_or_else(|_| "worker-1".into()))
//!     .auto_registry()
//!     .worker(std::env::var("CHRONON_WORKER_POOL").unwrap_or_else(|_| "general".into()))
//!     .build()?;
//! chronon.run().await?;
//! ```
//!
//! Runnable: `cargo run -p uf-chronon --example worker_daemon --features postgres,redis`

mod composite;
mod queue;

pub use composite::PostgresRedisSchedulerStore;
pub use queue::RedisQueueLayer;
