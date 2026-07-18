//! Script execution context ports.
//!
//! When a job is scheduled, Chronon stores `actor_json` on the job record. When a worker
//! dispatches the script, it calls [`ContextFactory::build`] to reconstruct handler context from
//! that JSON. Script handlers (including those defined with `#[chronon::script]`) receive the
//! result as `Box<dyn ScriptContext>`.
//!
//! # Choosing a factory
//!
//! | Approach | When to use |
//! |----------|-------------|
//! | [`JsonScriptContextFactory`] | Examples, smoke tests, and handlers that only need [`ScriptContext::label`] and [`ScriptContext::actor_json`] |
//! | Custom [`ContextFactory`] | Production apps that map actor JSON to sessions, permissions, database access, or other application identity |
//!
//! Install the factory at runtime boot on `ChrononBuilder` (see `chronon-runtime`).

use serde_json::Value;

use crate::error::{ChrononError, Result};

/// Opaque execution context for script handlers.
///
/// The runtime passes this as the first argument to `#[chronon::script]` handlers and to
/// registered invoke functions. Use [`Self::label`] for logs and [`Self::actor_json`] when the
/// handler only needs the captured actor payload from [`crate::models::Job::actor_json`].
///
/// # Examples
///
/// ```
/// use chronon_core::{ContextFactory, JsonScriptContextFactory, ScriptContext};
/// use serde_json::json;
///
/// let ctx = JsonScriptContextFactory
///     .build(&json!({"user": "alice"}))
///     .unwrap();
/// assert!(ctx.label().contains("alice"));
/// assert_eq!(ctx.actor_json()["user"], "alice");
/// ```
pub trait ScriptContext: Send {
    /// Debug label for logs and tests.
    fn label(&self) -> &str;

    /// Actor JSON captured at schedule time and restored at dispatch.
    fn actor_json(&self) -> &Value;
}

/// Builds a [`ScriptContext`] from JSON captured at schedule time.
///
/// Install on `ChrononBuilder::context_factory` at boot. The executor calls [`Self::build`] for
/// every dispatched run using the job's `actor_json`.
///
/// | Implementation | When to use |
/// |----------------|-------------|
/// | [`JsonScriptContextFactory`] | Examples / handlers that only need label + actor JSON |
/// | [`NoOpContextFactory`] | Tests and benches |
/// | Custom | Production identity, sessions, permissions |
///
/// # Examples
///
/// Custom factory sketch:
///
/// ```
/// use chronon_core::{ContextFactory, Result, ScriptContext};
/// use serde_json::Value;
///
/// struct AppCtx { label: String, actor_json: Value }
/// impl ScriptContext for AppCtx {
///     fn label(&self) -> &str { &self.label }
///     fn actor_json(&self) -> &Value { &self.actor_json }
/// }
///
/// struct AppFactory;
/// impl ContextFactory for AppFactory {
///     fn build(&self, actor_json: &Value) -> Result<Box<dyn ScriptContext>> {
///         Ok(Box::new(AppCtx {
///             label: actor_json.get("user").and_then(|v| v.as_str()).unwrap_or("anon").into(),
///             actor_json: actor_json.clone(),
///         }))
///     }
/// }
///
/// let ctx = AppFactory.build(&serde_json::json!({"user": "bob"})).unwrap();
/// assert_eq!(ctx.label(), "bob");
/// ```
pub trait ContextFactory: Send + Sync {
    /// Reconstruct handler context from actor JSON stored on the job.
    ///
    /// Returns [`IdentityError`] (mapped to [`ChrononError::Internal`]) when the payload
    /// cannot be decoded into application identity.
    fn build(&self, actor_json: &Value) -> Result<Box<dyn ScriptContext>>;
}

/// Identity reconstruction failure.
#[derive(Debug, thiserror::Error)]
#[error("identity error: {0}")]
pub struct IdentityError(pub String);

impl From<IdentityError> for ChrononError {
    fn from(value: IdentityError) -> Self {
        Self::Internal(value.0)
    }
}

/// No-op context for tests and benchmarks.
#[derive(Debug, Default)]
pub struct NoOpScriptContext {
    actor_json: Value,
}

impl ScriptContext for NoOpScriptContext {
    fn label(&self) -> &'static str {
        "noop"
    }

    fn actor_json(&self) -> &Value {
        &self.actor_json
    }
}

/// Factory that always returns [`NoOpScriptContext`].
#[derive(Debug, Default, Clone, Copy)]
pub struct NoOpContextFactory;

impl ContextFactory for NoOpContextFactory {
    fn build(&self, _actor_json: &Value) -> Result<Box<dyn ScriptContext>> {
        Ok(Box::new(NoOpScriptContext::default()))
    }
}

/// Default factory that wraps actor JSON in a labeled [`ScriptContext`].
///
/// Suitable for examples and handlers that only need [`ScriptContext::label`] and
/// [`ScriptContext::actor_json`]. For application-specific identity (database sessions,
/// permission checks, typed actors), implement [`ContextFactory`] instead.
///
/// # Examples
///
/// ```
/// use chronon_core::{ContextFactory, JsonScriptContextFactory};
/// use serde_json::json;
///
/// let factory = JsonScriptContextFactory;
/// let ctx = factory.build(&json!({"user": "alice"})).unwrap();
/// assert!(ctx.label().contains("alice"));
/// assert_eq!(ctx.actor_json()["user"], "alice");
/// ```
#[derive(Debug, Default, Clone, Copy)]
pub struct JsonScriptContextFactory;

struct JsonContext {
    actor_json: Value,
    label: String,
}

impl ScriptContext for JsonContext {
    fn label(&self) -> &str {
        &self.label
    }

    fn actor_json(&self) -> &Value {
        &self.actor_json
    }
}

impl ContextFactory for JsonScriptContextFactory {
    fn build(&self, actor_json: &Value) -> Result<Box<dyn ScriptContext>> {
        Ok(Box::new(JsonContext {
            actor_json: actor_json.clone(),
            label: actor_json.to_string(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    struct TestContext {
        label: String,
        actor_json: Value,
    }

    impl ScriptContext for TestContext {
        fn label(&self) -> &str {
            &self.label
        }

        fn actor_json(&self) -> &Value {
            &self.actor_json
        }
    }

    struct TestFactory;

    impl ContextFactory for TestFactory {
        fn build(&self, actor_json: &Value) -> Result<Box<dyn ScriptContext>> {
            if actor_json.get("System").is_some() {
                Ok(Box::new(TestContext {
                    label: "system".into(),
                    actor_json: actor_json.clone(),
                }))
            } else {
                Err(IdentityError("missing System".into()).into())
            }
        }
    }

    #[test]
    fn factory_builds_context() {
        let factory = TestFactory;
        let actor = json!({"System": {"operation": "t"}});
        let ctx = factory.build(&actor).expect("ok");
        assert_eq!(ctx.label(), "system");
        assert_eq!(ctx.actor_json(), &actor);
    }

    #[test]
    fn json_factory_stores_actor_json() {
        let factory = JsonScriptContextFactory;
        let actor = json!({"user": "alice"});
        let ctx = factory.build(&actor).expect("ok");
        assert_eq!(ctx.actor_json(), &actor);
        assert!(ctx.label().contains("alice"));
    }
}
