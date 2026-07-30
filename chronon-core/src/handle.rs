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
/// **Preferred scheduling:** build jobs with Chronon's fluent `JobBuilder`
/// (`chronon_scheduler::JobBuilder`, re-exported from the `chronon` / `uf-chronon` facade)
/// from this handle, then upsert via `CoordinatorService` or `RemoteCoordinatorClient`.
/// [`Self::job`] / [`Self::job_with_params`] only seed `script_name` / `params_json` — a
/// low-level alternate to the fluent builder.
///
/// # Examples
///
/// Handle identity (construction lives on Chronon `JobBuilder` — see
/// `cargo run -p uf-chronon --example script_handle_job --features mem`):
///
/// ```
/// use chronon_core::ScriptHandle;
/// use serde::Serialize;
///
/// #[derive(Serialize)]
/// struct NightlyCleanupParams {
///     retention_days: u32,
/// }
///
/// let handle = ScriptHandle::<NightlyCleanupParams>::new("nightly_cleanup");
/// assert_eq!(handle.name(), "nightly_cleanup");
/// ```
///
/// Low-level seed (prefer `JobBuilder::params` in application code):
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
    /// Prefer Chronon `JobBuilder` (`chronon_scheduler::JobBuilder`) for cron / run-once /
    /// manual scheduling. This method only seeds `job_name` and `script_name`.
    pub fn job(&self, job_name: impl Into<String>) -> Job {
        Job::new(job_name, self.name)
    }
}

impl<P: Serialize> ScriptHandle<P> {
    /// Baseline [`Job`] with typed params serialized into `params_json`.
    ///
    /// Prefer Chronon `JobBuilder::params` for fluent construction.
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
    fn job_seeds_script_name() {
        let handle = ScriptHandle::<()>::new("demo");
        let job = handle.job("demo-job");
        assert_eq!(job.script_name, "demo");
        assert_eq!(job.job_name, "demo-job");
    }

    #[test]
    fn job_with_params_serializes() {
        let handle = ScriptHandle::<DemoParams>::new("demo");
        let job = handle
            .job_with_params("demo-job", &DemoParams { n: 3 })
            .expect("serialize");
        assert_eq!(job.params_json["n"], 3);
    }
}
