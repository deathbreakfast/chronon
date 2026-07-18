//! In-memory [`SchedulerStore`](chronon_core::SchedulerStore) for tests and local development.
//!
//! Process-local, **non-durable** storage. Suitable for **Mode 1 embedded** boots, examples,
//! e2e, and benchmarks — **not** for multi-process Mode 2 clusters (`mem` does not cross
//! process boundaries).
//!
//! Getting started on the facade:
//! [Mode 1 — Embedded](https://docs.rs/uf-chronon/latest/chronon/index.html#mode-1--embedded-one-binary).
//!
//! ## Entry points
//!
//! - [`InMemorySchedulerStore::new`] — create a standalone store
//! - [`install_default_mem_store`] — register as the process-global default on [`StoreRouter`]
//!
//! Enable the `mem` feature on the `chronon` facade to re-export these types.
//!
//! ## Mode 1 — Embedded
//!
//! Wire with `ChrononBuilder::scheduler_store(Arc::new(InMemorySchedulerStore::new())).embedded()`
//! on the `chronon` facade (`mem` feature).
//!
//! ```ignore
//! use std::sync::Arc;
//! use chronon::prelude::*;
//! use chronon::InMemorySchedulerStore;
//!
//! let chronon = ChrononBuilder::new()
//!     .scheduler_store(Arc::new(InMemorySchedulerStore::new()))
//!     .context_factory(Arc::new(JsonScriptContextFactory))
//!     .embedded()
//!     .auto_registry()
//!     .build()?;
//! ```
//!
//! Runnable: `cargo run -p uf-chronon --example script_macro --features mem`.

mod store;

#[cfg(test)]
mod store_tests;

pub use store::InMemorySchedulerStore;

use std::sync::Arc;

use chronon_core::{DEFAULT_STORE_NAME, StoreRouter};

/// Registers a new in-memory store as the global default.
///
/// Returns the shared store handle so callers can pre-seed jobs or pass it to
/// `ChrononBuilder::scheduler_store` in `chronon-runtime`.
///
/// # Examples
///
/// ```
/// use chronon_backend_mem::install_default_mem_store;
/// use chronon_core::default_store_from_global;
///
/// let _store = install_default_mem_store();
/// assert!(default_store_from_global().is_ok());
/// ```
pub fn install_default_mem_store() -> Arc<InMemorySchedulerStore> {
    let store = Arc::new(InMemorySchedulerStore::new());
    StoreRouter::register_global(DEFAULT_STORE_NAME, store.clone());
    store
}
