//! `SQLite` [`SchedulerStore`](chronon_core::store::SchedulerStore) for Chronon.
//!
//! **Audience:** backend engineers and test authors needing embedded, file-backed persistence.
//!
//! ## Stack position
//!
//! ```text
//! chronon (facade, `sqlite` feature) → chronon-backend-sqlite → chronon-backend-sql-common → chronon-core
//! ```
//!
//! Suitable for single-process deployments and CI. SQLite allows one writer at a time;
//! use PostgreSQL or the Redis composite backend for multi-worker claim throughput.
//!
//! ## Entry points
//!
//! - [`SqliteSchedulerStore::new`] — open a database file
//! - [`SqliteSchedulerStore::connect`] — connect via URL (including `:memory:`)
//!
//! ## Example
//!
//! ```rust,no_run
//! use chronon_backend_sqlite::SqliteSchedulerStore;
//!
//! # async fn example() -> chronon_core::Result<()> {
//! let store = SqliteSchedulerStore::connect("sqlite://:memory:").await?;
//! # Ok(())
//! # }
//! ```

use std::path::Path;

use chronon_backend_sql_common::SqlSchedulerStore;
use chronon_core::Result;
use sqlx::SqlitePool;

/// SQLite-backed scheduler store.
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
