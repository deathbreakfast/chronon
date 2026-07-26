//! Shared state for Chronon API handlers.

use std::sync::Arc;

use chronon_core::{ChrononError, Result};
use chronon_executor::ScriptRegistry;
use chronon_runtime::CoordinatorService;
use serde_json::Value as JsonValue;

use crate::auth::{require_admin_auth_from_env, AdminAuth};

/// Callback that supplies `actor_json` when HTTP upsert omits a client actor.
pub type HttpUpsertActorProvider = Arc<dyn Fn() -> JsonValue + Send + Sync>;

/// Shared state for Chronon API handlers.
///
/// Install on the host router via [`axum::extract::FromRef`] and pass to [`crate::chronon_router`].
/// Construct with [`ChrononState::new`] or [`ChrononState::builder`].
#[derive(Clone)]
pub struct ChrononState {
    /// Job and run persistence API.
    pub coordinator: Arc<CoordinatorService>,
    /// Script catalog for upsert validation and `GET /scripts`.
    pub registry: Arc<ScriptRegistry>,
    /// Optional host verifier for admin routes.
    pub admin_auth: Option<Arc<dyn AdminAuth>>,
    /// When true, requests are rejected if [`admin_auth`](Self::admin_auth) is `None`.
    pub require_admin_auth: bool,
    /// Optional override for default HTTP upsert actor JSON (when request omits actor).
    pub http_upsert_actor: Option<HttpUpsertActorProvider>,
}

impl ChrononState {
    /// Build handler state from coordinator and registry (no admin auth; require-flag from env).
    #[must_use]
    pub fn new(coordinator: Arc<CoordinatorService>, registry: Arc<ScriptRegistry>) -> Self {
        Self {
            coordinator,
            registry,
            admin_auth: None,
            require_admin_auth: require_admin_auth_from_env(),
            http_upsert_actor: None,
        }
    }

    /// Builder for authenticated / customized admin mounts.
    #[must_use]
    pub fn builder(
        coordinator: Arc<CoordinatorService>,
        registry: Arc<ScriptRegistry>,
    ) -> ChrononStateBuilder {
        ChrononStateBuilder {
            coordinator,
            registry,
            admin_auth: None,
            require_admin_auth: require_admin_auth_from_env(),
            http_upsert_actor: None,
        }
    }

    /// Actor JSON used when HTTP upsert omits `actor_json`.
    #[must_use]
    pub fn default_upsert_actor_json(&self) -> JsonValue {
        if let Some(ref provider) = self.http_upsert_actor {
            return provider();
        }
        chronon_core::default_http_enqueue_actor()
    }
}

/// Build [`ChrononState`] with admin auth and actor overrides.
pub struct ChrononStateBuilder {
    coordinator: Arc<CoordinatorService>,
    registry: Arc<ScriptRegistry>,
    admin_auth: Option<Arc<dyn AdminAuth>>,
    require_admin_auth: bool,
    http_upsert_actor: Option<HttpUpsertActorProvider>,
}

impl ChrononStateBuilder {
    /// Install a host [`AdminAuth`] verifier.
    #[must_use]
    pub fn admin_auth(mut self, auth: Arc<dyn AdminAuth>) -> Self {
        self.admin_auth = Some(auth);
        self
    }

    /// Force require-admin-auth (overrides env when set).
    #[must_use]
    pub const fn require_admin_auth(mut self, require: bool) -> Self {
        self.require_admin_auth = require;
        self
    }

    /// Override default HTTP upsert actor JSON (when the request omits actor).
    #[must_use]
    pub fn http_upsert_actor(
        mut self,
        provider: impl Fn() -> JsonValue + Send + Sync + 'static,
    ) -> Self {
        self.http_upsert_actor = Some(Arc::new(provider));
        self
    }

    /// Build state.
    ///
    /// # Errors
    ///
    /// Returns [`ChrononError::Internal`] when `require_admin_auth` is set and no verifier
    /// was installed.
    pub fn build(self) -> Result<ChrononState> {
        if self.require_admin_auth && self.admin_auth.is_none() {
            return Err(ChrononError::Internal(
                "CHRONON_REQUIRE_ADMIN_AUTH is set but no AdminAuth verifier was configured".into(),
            ));
        }
        Ok(ChrononState {
            coordinator: self.coordinator,
            registry: self.registry,
            admin_auth: self.admin_auth,
            require_admin_auth: self.require_admin_auth,
            http_upsert_actor: self.http_upsert_actor,
        })
    }
}
