//! `PostgreSQL` [`SchedulerStore`](chronon_core::store::SchedulerStore) for Chronon.
//!
//! **When to use:** shared durable storage for **embedded** or **coordinator–worker** clusters.
//! For higher claim throughput, wrap with `PostgresRedisSchedulerStore`
//! (`chronon-backend-redis`).
//!
//! Getting started:
//! [Embedded](https://docs.rs/uf-chronon/latest/chronon/index.html#embedded-one-process) /
//! [Coordinator–worker](https://docs.rs/uf-chronon/latest/chronon/index.html#coordinator-worker-split).
//!
//! ## Stack position
//!
//! ```text
//! chronon (public crate, `postgres` feature) → chronon-backend-postgres → chronon-backend-sql-common → chronon-core
//! ```
//!
//! ## Entry points
//!
//! - [`PostgresSchedulerStore::connect`] — open a pool and bootstrap schema
//! - [`PostgresSchedulerStore::connect_isolated`] — isolated schema for parallel tests
//! - [`postgres_test_url`] — resolve test URL from `CHRONON_POSTGRES_URL` / `CHRONON_TEST_POSTGRES_URL`
//!
//! ## Embedded
//!
//! ```ignore
//! use std::sync::Arc;
//! use chronon::prelude::*;
//! use chronon::PostgresSchedulerStore;
//!
//! let url = std::env::var("CHRONON_POSTGRES_URL")?;
//! let store = PostgresSchedulerStore::connect(&url).await?;
//! let chronon = ChrononBuilder::new()
//!     .scheduler_store(Arc::new(store))
//!     .context_factory(Arc::new(JsonScriptContextFactory))
//!     .embedded()
//!     .auto_registry()
//!     .build()?;
//! ```
//!
//! Runnable: `cargo run -p uf-chronon --example postgres_boot --features postgres`
//!
//! ## Coordinator binary
//!
//! Shared `CHRONON_POSTGRES_URL` with workers. Tick only:
//!
//! ```ignore
//! use std::sync::Arc;
//! use chronon::prelude::*;
//! use chronon::PostgresSchedulerStore;
//!
//! let url = std::env::var("CHRONON_POSTGRES_URL")?;
//! let store = PostgresSchedulerStore::connect(&url).await?;
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
//! Runnable: `cargo run -p uf-chronon --example postgres_coordinator_daemon --features postgres`
//!
//! ## Worker binary
//!
//! Same Postgres URL, unique `CHRONON_INSTANCE_ID`, scripts via `.auto_registry()`:
//!
//! ```ignore
//! use std::sync::Arc;
//! use chronon::prelude::*;
//! use chronon::PostgresSchedulerStore;
//!
//! let url = std::env::var("CHRONON_POSTGRES_URL")?;
//! let store = PostgresSchedulerStore::connect(&url).await?;
//! let mut chronon = ChrononBuilder::new()
//!     .scheduler_store(Arc::new(store))
//!     .context_factory(Arc::new(JsonScriptContextFactory))
//!     .instance_id(std::env::var("CHRONON_INSTANCE_ID").unwrap_or_else(|_| "worker-1".into()))
//!     .auto_registry()
//!     .worker("general")
//!     .build()?;
//! chronon.run().await?;
//! ```
//!
//! Runnable: `cargo run -p uf-chronon --example postgres_worker_daemon --features postgres`
//!
//! Production claim path: [Postgres + Redis](../chronon_backend_redis/index.html#coordinator-binary).

mod bootstrap;

use chronon_backend_sql_common::SqlSchedulerStore;
use chronon_core::Result;
use sqlx::PgPool;

pub use bootstrap::{postgres_store_from_env, postgres_test_url};

/// PostgreSQL-backed scheduler store.
///
/// Shared durable storage for coordinator–worker clusters (and embedded when you already run
/// Postgres). Pass a connection URL to [`Self::connect`]; daemons often use
/// `CHRONON_POSTGRES_URL` / [`postgres_test_url`].
///
/// Split examples: [coordinator](index.html#coordinator-binary) /
/// [worker](index.html#worker-binary).
///
/// For higher claim throughput, wrap with `PostgresRedisSchedulerStore` from
/// `chronon-backend-redis` (`postgres` + `redis` features). Enable the public crate `postgres`
/// feature to re-export this type.
///
/// # Examples
///
/// ```rust,no_run
/// use chronon_backend_postgres::PostgresSchedulerStore;
///
/// # async fn example() -> chronon_core::Result<()> {
/// let store = PostgresSchedulerStore::connect(
///     "postgres://user:pass@localhost/chronon",
/// )
/// .await?;
/// # let _ = store;
/// # Ok(())
/// # }
/// ```
///
/// Runnable: `cargo run -p uf-chronon --example postgres_boot --features postgres`.
pub struct PostgresSchedulerStore {
    inner: SqlSchedulerStore,
}

impl PostgresSchedulerStore {
    /// Connect using a `PostgreSQL` connection URL.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use chronon_backend_postgres::PostgresSchedulerStore;
    ///
    /// # async fn example() -> chronon_core::Result<()> {
    /// let store = PostgresSchedulerStore::connect(
    ///     "postgres://user:pass@localhost/chronon",
    /// )
    /// .await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns a storage error when the pool cannot connect or schema bootstrap fails.
    pub async fn connect(url: &str) -> Result<Self> {
        let inner = SqlSchedulerStore::connect_postgres(url).await?;
        Ok(Self { inner })
    }

    /// Connect with an isolated schema (for parallel tests).
    ///
    /// `schema` must be an allowlisted identifier (`^[A-Za-z_][A-Za-z0-9_]*$`, max 63 chars)
    /// or connect fails with [`chronon_core::ChrononError::ParamError`] before any DDL.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use chronon_backend_postgres::PostgresSchedulerStore;
    ///
    /// # async fn example() -> chronon_core::Result<()> {
    /// let store = PostgresSchedulerStore::connect_isolated(
    ///     "postgres://user:pass@localhost/chronon",
    ///     "chronon_test_schema",
    /// )
    /// .await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns a parameter error for invalid schema names, or a storage error when schema
    /// creation, pool connect, or bootstrap fails.
    pub async fn connect_isolated(url: &str, schema: &str) -> Result<Self> {
        let inner = SqlSchedulerStore::connect_postgres_isolated(url, schema).await?;
        Ok(Self { inner })
    }

    /// Attach to an existing isolated schema (no DDL bootstrap; for multi-process workers).
    ///
    /// # Errors
    ///
    /// Returns a storage error when the pool cannot be opened.
    pub async fn attach_isolated(url: &str, schema: &str) -> Result<Self> {
        let inner = SqlSchedulerStore::attach_postgres_isolated(url, schema).await?;
        Ok(Self { inner })
    }

    /// Drop an isolated bench/test schema (multibench cell reset).
    ///
    /// # Errors
    ///
    /// Returns a storage error when the admin connection or DDL fails.
    pub async fn drop_isolated_schema(url: &str, schema: &str) -> Result<()> {
        SqlSchedulerStore::drop_postgres_schema(url, schema).await
    }

    /// Wrap an existing pool (schema bootstrap runs).
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use chronon_backend_postgres::PostgresSchedulerStore;
    /// use sqlx::postgres::PgPoolOptions;
    ///
    /// # async fn example() -> chronon_core::Result<()> {
    /// let pool = PgPoolOptions::new()
    ///     .connect("postgres://localhost/chronon")
    ///     .await
    ///     .expect("pool");
    /// let store = PostgresSchedulerStore::from_pool(pool).await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns a storage error when schema bootstrap fails.
    pub async fn from_pool(pool: PgPool) -> Result<Self> {
        let inner = SqlSchedulerStore::from_postgres_pool(pool).await?;
        Ok(Self { inner })
    }

    /// Underlying connection pool.
    ///
    /// # Panics
    ///
    /// Panics if the inner pool is not `PostgreSQL` (internal invariant violation).
    #[must_use]
    pub fn pool(&self) -> &PgPool {
        match self.inner.pool() {
            chronon_backend_sql_common::SqlPool::Postgres(pool) => pool,
            chronon_backend_sql_common::SqlPool::Sqlite(_) => {
                panic!("postgres backend has non-postgres pool")
            }
        }
    }
}

impl std::fmt::Debug for PostgresSchedulerStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PostgresSchedulerStore")
            .finish_non_exhaustive()
    }
}

chronon_backend_sql_common::delegate_scheduler_store!(PostgresSchedulerStore, inner);
