//! Shared SQL [`SchedulerStore`](chronon_core::store::SchedulerStore) for `PostgreSQL` and `SQLite`.
//!
//! Backend and platform engineers use this crate when implementing or extending SQL persistence.
//! Application binaries should prefer the thin wrappers
//! [`chronon_backend_postgres`](https://docs.rs/chronon-backend-postgres) or
//! [`chronon_backend_sqlite`](https://docs.rs/chronon-backend-sqlite).
//!
//! ## Stack position
//!
//! ```text
//! chronon-backend-{postgres,sqlite} → chronon-backend-sql-common → chronon-core
//! ```
//!
//! ## Entry points
//!
//! - [`SqlSchedulerStore`] — connect, schema bootstrap, and trait implementation
//! - [`SqlDialect`] / [`SqlPool`] — engine selection and pool wrapper
//! - [`bind_sql`] — dialect-specific placeholder rewriting (`?` → `$1`, …)
//! - [`validate_postgres_schema_name`] — allowlist for isolated schema DDL / `search_path`
//!
//! ## Prerequisites
//!
//! Schema bootstrap runs on connect. For parallel Postgres tests use
//! [`SqlSchedulerStore::connect_postgres_isolated`] (schema names must match
//! [`validate_postgres_schema_name`]).
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

pub use backend::{
    bind_sql, validate_postgres_schema_name, SqlDialect, SqlPool, SqlSchedulerStore,
};
pub use coordinator::LEADER_ROW_ID;
pub use row::run_pool_key;
pub use row::{row_to_worker, SchedulerLeaderRow};
