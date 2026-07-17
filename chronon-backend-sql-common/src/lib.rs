//! Shared SQL [`SchedulerStore`](chronon_core::store::SchedulerStore) for `PostgreSQL` and `SQLite`.
//!
//! **Audience:** backend and platform engineers implementing or extending SQL persistence.
//!
//! ## Stack position
//!
//! ```text
//! chronon-backend-{postgres,sqlite} → chronon-backend-sql-common → chronon-core
//! ```
//!
//! Integrators typically use the thin wrapper crates
//! [`chronon_backend_postgres`](https://docs.rs/chronon-backend-postgres) or
//! [`chronon_backend_sqlite`](https://docs.rs/chronon-backend-sqlite) rather than
//! depending on this crate directly.
//!
//! ## Entry points
//!
//! - [`SqlSchedulerStore`] — connect, schema bootstrap, and trait implementation
//! - [`SqlDialect`] / [`SqlPool`] — engine selection and pool wrapper
//! - [`bind_sql`] — dialect-specific placeholder rewriting (`?` → `$1`, …)
//!
//! ## Prerequisites
//!
//! Schema bootstrap runs on connect. For parallel Postgres tests use
//! [`SqlSchedulerStore::connect_postgres_isolated`].
//!
//! ## Example
//!
//! ```rust,no_run
//! use chronon_backend_sql_common::SqlSchedulerStore;
//!
//! # async fn example() -> chronon_core::Result<()> {
//! let store = SqlSchedulerStore::connect_sqlite("sqlite://:memory:").await?;
//! # Ok(())
//! # }
//! ```

mod backend;
mod claims;
mod coordinator;
mod delegate;
mod error_map;
mod jobs;
mod macros;
mod row;
mod runs;
mod schema;
mod store_impl;

#[cfg(test)]
mod store_smoke;

pub use backend::{bind_sql, SqlDialect, SqlPool, SqlSchedulerStore};
pub use coordinator::LEADER_ROW_ID;
pub use row::run_pool_key;
pub use row::{row_to_worker, SchedulerLeaderRow};
