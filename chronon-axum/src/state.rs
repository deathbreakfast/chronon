//! Shared state for Chronon API handlers.

use std::sync::Arc;

use chronon_executor::ScriptRegistry;
use chronon_runtime::CoordinatorService;

/// Shared state for Chronon API handlers.
///
/// Install on the host router via [`axum::extract::FromRef`] and pass to [`crate::chronon_router`].
#[derive(Clone)]
pub struct ChrononState {
    /// Job and run persistence facade.
    pub coordinator: Arc<CoordinatorService>,
    /// Script catalog for upsert validation and `GET /scripts`.
    pub registry: Arc<ScriptRegistry>,
}

impl ChrononState {
    /// Build handler state from coordinator and registry arcs.
    pub fn new(coordinator: Arc<CoordinatorService>, registry: Arc<ScriptRegistry>) -> Self {
        Self {
            coordinator,
            registry,
        }
    }
}
