//! Named [`SchedulerStore`] registration at host boot.
//!
//! Register one or more backends under logical names before constructing
//! `Chronon` in `chronon-runtime`. Use [`StoreRouter::register_global`] with
//! [`DEFAULT_STORE_NAME`] for single-store setups.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};

use crate::error::{ChrononError, Result};
use crate::store::SchedulerStore;

/// Default logical store name when hosts register a single backend.
pub const DEFAULT_STORE_NAME: &str = "default";

static GLOBAL_ROUTER: OnceLock<RwLock<StoreRouter>> = OnceLock::new();

fn global_router() -> &'static RwLock<StoreRouter> {
    GLOBAL_ROUTER.get_or_init(|| RwLock::new(StoreRouter::new()))
}

/// Registers named [`SchedulerStore`] backends at host boot.
///
/// Thread-safe when accessed through [`Self::register_global`] / [`default_store_from_global`];
/// direct mutation requires exclusive access to the router instance.
#[derive(Default)]
pub struct StoreRouter {
    stores: HashMap<String, Arc<dyn SchedulerStore>>,
}

impl StoreRouter {
    /// Create an empty router (no stores registered).
    pub fn new() -> Self {
        Self {
            stores: HashMap::new(),
        }
    }

    /// Register a store under a logical name (overwrites any previous entry).
    pub fn register(&mut self, name: impl Into<String>, store: Arc<dyn SchedulerStore>) {
        self.stores.insert(name.into(), store);
    }

    /// Resolve a registered store by name.
    pub fn get(&self, name: &str) -> Option<Arc<dyn SchedulerStore>> {
        self.stores.get(name).cloned()
    }

    /// Replace the process-global router (typically once at startup).
    ///
    /// Subsequent calls are ignored; the first successful install wins.
    pub fn install_global(router: Self) {
        let _ = GLOBAL_ROUTER.set(RwLock::new(router));
    }

    /// Register a store on the process-global router.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::sync::Arc;
    /// # use chronon_core::{SchedulerStore, StoreRouter, DEFAULT_STORE_NAME};
    /// # fn demo(store: Arc<dyn SchedulerStore>) {
    /// StoreRouter::register_global(DEFAULT_STORE_NAME, store);
    /// # }
    /// ```
    ///
    /// Panics if the global router lock is poisoned.
    pub fn register_global(name: impl Into<String>, store: Arc<dyn SchedulerStore>) {
        global_router()
            .write()
            .expect("StoreRouter lock poisoned")
            .register(name, store);
    }
}

/// Resolves the default store from the global router.
///
/// Returns [`ChrononError::StorageError`] when no store is registered under
/// [`DEFAULT_STORE_NAME`].
pub fn default_store_from_global() -> Result<Arc<dyn SchedulerStore>> {
    global_router()
        .read()
        .expect("StoreRouter lock poisoned")
        .get(DEFAULT_STORE_NAME)
        .ok_or_else(|| ChrononError::StorageError("no default SchedulerStore registered".into()))
}
