//! `SQLite` [`SchedulerStore`](chronon_core::store::SchedulerStore) for Chronon.
//!
//! **When to use:** durable single-host **Mode 1**, or same-host **Mode 2** when coordinator and
//! worker open the **same database file** (SQLite allows one writer at a time — prefer Postgres
//! ± Redis for multi-worker fleets).
//!
//! Getting started:
//! [Mode 1](https://docs.rs/uf-chronon/latest/chronon/index.html#mode-1--embedded-one-binary) /
//! [Mode 2](https://docs.rs/uf-chronon/latest/chronon/index.html#mode-2--coordinator--worker-two-binaries).
//!
//! ## Stack position
//!
//! ```text
//! chronon (facade, `sqlite` feature) → chronon-backend-sqlite → chronon-backend-sql-common → chronon-core
//! ```
//!
//! ## Entry points
//!
//! - [`SqliteSchedulerStore::new`] — open a database file
//! - [`SqliteSchedulerStore::connect`] — connect via URL (including `:memory:`)
//!
//! ## Mode 1 — Embedded
//!
//! ```ignore
//! use std::sync::Arc;
//! use chronon::prelude::*;
//! use chronon::SqliteSchedulerStore;
//!
//! let store = SqliteSchedulerStore::connect("sqlite:///var/lib/chronon/chronon.db").await?;
//! let chronon = ChrononBuilder::new()
//!     .scheduler_store(Arc::new(store))
//!     .context_factory(Arc::new(JsonScriptContextFactory))
//!     .embedded()
//!     .auto_registry()
//!     .build()?;
//! ```
//!
//! Runnable: `cargo run -p uf-chronon --example sqlite_boot --features sqlite`
//!
//! ## Mode 2 — Coordinator binary
//!
//! Shared file path with the worker. Tick only — no script execution in this process:
//!
//! ```ignore
//! use std::sync::Arc;
//! use chronon::prelude::*;
//! use chronon::SqliteSchedulerStore;
//!
//! let path = std::env::var("CHRONON_SQLITE_PATH")
//!     .unwrap_or_else(|_| "/tmp/chronon-remote.db".into());
//! let store = SqliteSchedulerStore::new(&path).await?;
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
//! ## Mode 2 — Worker binary
//!
//! Same `CHRONON_SQLITE_PATH`, unique `CHRONON_INSTANCE_ID`, scripts linked via `.auto_registry()`:
//!
//! ```ignore
//! use std::sync::Arc;
//! use chronon::prelude::*;
//! use chronon::SqliteSchedulerStore;
//!
//! let path = std::env::var("CHRONON_SQLITE_PATH")
//!     .unwrap_or_else(|_| "/tmp/chronon-remote.db".into());
//! let store = SqliteSchedulerStore::new(&path).await?;
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
//! Other Mode 2 backends:
//! [Postgres](../chronon_backend_postgres/index.html#mode-2--coordinator-binary),
//! [Postgres + Redis](../chronon_backend_redis/index.html#mode-2--coordinator-binary).

use std::path::Path;

use chronon_backend_sql_common::SqlSchedulerStore;
use chronon_core::Result;
use sqlx::SqlitePool;

/// SQLite-backed scheduler store.
///
/// Durable **single-host** persistence for Mode 1 embedded deployments and CI. SQLite allows
/// one writer at a time — prefer Postgres (+ Redis) for multi-worker Mode 2 claim throughput.
/// Same-host Mode 2 is possible when both binaries open the **same path**.
///
/// Mode 2 examples: [coordinator](index.html#mode-2--coordinator-binary) /
/// [worker](index.html#mode-2--worker-binary).
///
/// Enable the facade `sqlite` feature. Construct with [`Self::new`] (file path) or
/// [`Self::connect`] (URL, including `sqlite://:memory:`).
///
/// # Examples
///
/// ```rust,no_run
/// use chronon_backend_sqlite::SqliteSchedulerStore;
///
/// # async fn example() -> chronon_core::Result<()> {
/// let store = SqliteSchedulerStore::connect("sqlite://:memory:").await?;
/// # let _ = store;
/// # Ok(())
/// # }
/// ```
///
/// Runnable: `cargo run -p uf-chronon --example sqlite_boot --features sqlite`.
pub struct SqliteSchedulerStore {
    inner: SqlSchedulerStore,
}

impl SqliteSchedulerStore {
    /// Open a `SQLite` database at `path` (creates the file if missing).
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use chronon_backend_sqlite::SqliteSchedulerStore;
    ///
    /// # async fn example() -> chronon_core::Result<()> {
    /// let store = SqliteSchedulerStore::new("/var/lib/chronon/chronon.db").await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns a storage error when the database cannot be opened or schema bootstrap fails.
    pub async fn new(path: impl AsRef<Path>) -> Result<Self> {
        let url = format!("sqlite://{}?mode=rwc", path.as_ref().display());
        Self::connect(&url).await
    }

    /// Connect using a `SQLite` connection URL.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use chronon_backend_sqlite::SqliteSchedulerStore;
    ///
    /// # async fn example() -> chronon_core::Result<()> {
    /// let store = SqliteSchedulerStore::connect("sqlite://:memory:").await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns a storage error when the pool cannot connect or schema bootstrap fails.
    pub async fn connect(url: &str) -> Result<Self> {
        let inner = SqlSchedulerStore::connect_sqlite(url).await?;
        Ok(Self { inner })
    }

    /// Wrap an existing pool (schema bootstrap runs).
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use chronon_backend_sqlite::SqliteSchedulerStore;
    /// use sqlx::sqlite::SqlitePoolOptions;
    ///
    /// # async fn example() -> chronon_core::Result<()> {
    /// let pool = SqlitePoolOptions::new()
    ///     .connect("sqlite://:memory:")
    ///     .await
    ///     .expect("pool");
    /// let store = SqliteSchedulerStore::from_pool(pool).await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns a storage error when schema bootstrap fails.
    pub async fn from_pool(pool: SqlitePool) -> Result<Self> {
        let inner = SqlSchedulerStore::from_sqlite_pool(pool).await?;
        Ok(Self { inner })
    }

    /// Underlying connection pool.
    ///
    /// # Panics
    ///
    /// Panics if the inner pool is not `SQLite` (internal invariant violation).
    #[must_use]
    pub fn pool(&self) -> &SqlitePool {
        match self.inner.pool() {
            chronon_backend_sql_common::SqlPool::Sqlite(pool) => pool,
            chronon_backend_sql_common::SqlPool::Postgres(_) => {
                panic!("sqlite backend has non-sqlite pool")
            }
        }
    }
}

impl std::fmt::Debug for SqliteSchedulerStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SqliteSchedulerStore")
            .finish_non_exhaustive()
    }
}

chronon_backend_sql_common::delegate_scheduler_store!(SqliteSchedulerStore, inner);
