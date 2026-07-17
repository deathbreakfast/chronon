//! Typed script handle for job scheduling.

use std::marker::PhantomData;

use serde::Serialize;

use crate::models::Job;
use crate::Result;

/// A typed handle for scheduling a script with specific parameters.
///
/// Created by the `#[chronon::script]` macro. The attribute turns the annotated
/// function into a handle factory (`fn nightly_cleanup() -> ScriptHandle<…>`) and
/// moves the body to an internal `__*_impl` entry point used by the executor.
///
/// # Examples
///
/// Build a default [`Job`] from the macro-generated handle, then upsert it:
///
/// ```
/// use chronon_core::{Job, ScriptHandle};
/// use serde::Serialize;
///
/// #[derive(Serialize)]
/// struct NightlyCleanupParams {
///     retention_days: u32,
/// }
///
/// let handle = ScriptHandle::<NightlyCleanupParams>::new("nightly_cleanup");
/// let job: Job = handle
///     .job_with_params(
///         "nightly-job",
///         &NightlyCleanupParams {
///             retention_days: 7,
///         },
///     )
///     .expect("params serialize");
/// assert_eq!(job.script_name, "nightly_cleanup");
/// assert_eq!(job.params_json["retention_days"], 7);
/// ```
///
/// Runnable end-to-end sample: `cargo run -p uf-chronon --example script_handle_job --features mem`.
#[derive(Debug, Clone)]
pub struct ScriptHandle<P> {
    name: &'static str,
    _params: PhantomData<P>,
}

impl<P> ScriptHandle<P> {
    /// Create a new script handle (typically called by macro-generated code).
    pub const fn new(name: &'static str) -> Self {
        Self {
            name,
            _params: PhantomData,
        }
    }

    /// Stable script registry name.
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// Baseline [`Job`] pointing at this script (`Job::new` defaults).
    ///
    /// Populate schedule fields (`schedule_kind`, `cron_expr`, `next_run_at`, …)
    /// before upserting via the coordinator service or HTTP API.
    pub fn job(&self, job_name: impl Into<String>) -> Job {
        Job::new(job_name, self.name)
    }
}

impl<P: Serialize> ScriptHandle<P> {
    /// Baseline [`Job`] with typed params serialized into `params_json`.
    pub fn job_with_params(&self, job_name: impl Into<String>, params: &P) -> Result<Job> {
        let mut job = self.job(job_name);
        job.params_json = serde_json::to_value(params).map_err(crate::ChrononError::from)?;
        Ok(job)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;

    #[derive(Serialize)]
    struct DemoParams {
        n: u32,
    }

    #[test]
    fn job_sets_script_name() {
        let handle = ScriptHandle::<()>::new("demo");
        let job = handle.job("demo-job");
        assert_eq!(job.job_name, "demo-job");
        assert_eq!(job.script_name, "demo");
    }

    #[test]
    fn job_with_params_serializes() {
        let handle = ScriptHandle::<DemoParams>::new("demo");
        let job = handle
            .job_with_params("demo-job", &DemoParams { n: 3 })
            .unwrap();
        assert_eq!(job.params_json["n"], 3);
    }
}
