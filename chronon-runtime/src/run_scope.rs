//! Per-run script context: the host's context plus the run's [`RunValues`].

use std::sync::Arc;

use chronon_core::{ContextFactory, Result, RunValues, ScriptContext};
use serde_json::Value;

/// Wraps the host's [`ScriptContext`] and answers [`ScriptContext::run`] with the values of the
/// run being executed.
///
/// The worker builds one per dispatched run: it calls the host's [`ContextFactory`] as before,
/// then wraps the result. `label` and `actor_json` come from the host context unchanged.
///
/// # Examples
///
/// ```
/// use chrono::Utc;
/// use chronon_core::{ContextFactory, Job, JsonScriptContextFactory, Run, RunValues, ScriptContext};
/// use chronon_runtime::RunScope;
/// use serde_json::json;
///
/// let job = Job::new("nightly", "cleanup");
/// let run = Run::for_job(&job.job_id, "cleanup", Utc::now());
/// let host = JsonScriptContextFactory.build(&json!({"user": "alice"})).unwrap();
/// let ctx = RunScope::new(host, RunValues::from_run(&run, &job));
/// assert_eq!(ctx.run().run.id, run.run_id);
/// assert_eq!(ctx.actor_json()["user"], "alice");
/// ```
pub struct RunScope {
    inner: Box<dyn ScriptContext>,
    values: RunValues,
}

impl RunScope {
    /// Wrap `inner` so it reports `values` from [`ScriptContext::run`].
    pub fn new(inner: Box<dyn ScriptContext>, values: RunValues) -> Self {
        Self { inner, values }
    }
}

impl ScriptContext for RunScope {
    fn label(&self) -> &str {
        self.inner.label()
    }

    fn actor_json(&self) -> &Value {
        self.inner.actor_json()
    }

    fn run(&self) -> &RunValues {
        &self.values
    }
}

/// [`ContextFactory`] for one run: builds the host context and wraps it in a [`RunScope`].
pub(crate) struct RunScopeFactory {
    inner: Arc<dyn ContextFactory>,
    values: RunValues,
}

impl RunScopeFactory {
    pub(crate) fn new(inner: Arc<dyn ContextFactory>, values: RunValues) -> Self {
        Self { inner, values }
    }
}

impl ContextFactory for RunScopeFactory {
    fn build(&self, actor_json: &Value) -> Result<Box<dyn ScriptContext>> {
        let inner = self.inner.build(actor_json)?;
        Ok(Box::new(RunScope::new(inner, self.values.clone())))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use chrono::Utc;
    use chronon_core::{ChrononError, Job, JsonScriptContextFactory, Run};
    use serde_json::json;

    struct RejectingFactory;

    impl ContextFactory for RejectingFactory {
        fn build(&self, _actor_json: &Value) -> Result<Box<dyn ScriptContext>> {
            Err(ChrononError::Identity("no".into()))
        }
    }

    fn values() -> (Run, RunValues) {
        let job = Job::new("j", "s");
        let run = Run::for_job(&job.job_id, "s", Utc::now());
        let values = RunValues::from_run(&run, &job);
        (run, values)
    }

    #[test]
    fn factory_wraps_host_context() {
        let (run, values) = values();
        let factory = RunScopeFactory::new(Arc::new(JsonScriptContextFactory), values);
        let ctx = factory.build(&json!({"user": "bob"})).unwrap();
        assert_eq!(ctx.run().run.id, run.run_id);
        assert_eq!(ctx.actor_json(), &json!({"user": "bob"}));
        assert!(ctx.label().contains("bob"));
    }

    #[test]
    fn factory_keeps_host_identity_errors() {
        let (_run, values) = values();
        let factory = RunScopeFactory::new(Arc::new(RejectingFactory), values);
        assert!(matches!(
            factory.build(&json!({})),
            Err(ChrononError::Identity(_))
        ));
    }
}
