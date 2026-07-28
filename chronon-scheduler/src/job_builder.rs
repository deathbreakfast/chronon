//! Fluent builder for constructing [`Job`](chronon_core::Job) rows (preferred schedule entry point).
//!
//! Owns cron/run-once/manual scheduling, params, pool/region, retry/misfire policy, and optional
//! opaque [`Job::actor_json`](chronon_core::Job::actor_json). Does **not** own Valence identity or
//! persistence — hosts snapshot actors and upsert via `CoordinatorService`,
//! `RemoteCoordinatorClient`, or an L1 coordinator backend.
//!
//! Prefer this over seeding with [`ScriptHandle::job`](chronon_core::ScriptHandle::job) /
//! [`job_with_params`](chronon_core::ScriptHandle::job_with_params) and mutating schedule fields
//! by hand.
//!
//! # Examples
//!
//! ```
//! use chronon_core::ScriptHandle;
//! use chronon_scheduler::JobBuilder;
//! use serde::Serialize;
//!
//! #[derive(Serialize)]
//! struct NightlyParams {
//!     retention_days: u32,
//! }
//!
//! let handle = ScriptHandle::<NightlyParams>::new("nightly_cleanup");
//! let job = JobBuilder::new(&handle)
//!     .name("nightly-cleanup")
//!     .cron("0 0 * * * *")?
//!     .timezone("UTC")
//!     .params(NightlyParams {
//!         retention_days: 7,
//!     })
//!     .with_actor_json(serde_json::json!({ "Service": { "name": "ops" } }))
//!     .build()?;
//!
//! assert_eq!(job.script_name, "nightly_cleanup");
//! assert!(job.next_run_at.is_some());
//! # Ok::<(), chronon_core::ChrononError>(())
//! ```

use chrono::{DateTime, Utc};
use chronon_core::{
    ChrononError, Job, MisfirePolicy, Result, RetryPolicy, ScheduleKind, ScriptHandle,
};
use serde::Serialize;
use serde_json::Value;

use crate::CronExpr;

/// Fluent builder for a scheduled [`Job`].
///
/// Requires [`Self::name`] before [`Self::build`]. Identity is optional via
/// [`Self::with_actor_json`] (default remains [`Job::new`](chronon_core::Job::new)'s `Null`).
///
/// # Errors
///
/// Fallible setters and [`Self::build`] return [`ChrononError`] variants documented on each method.
/// Error messages must not embed full `actor_json` or `params_json` payloads.
#[must_use = "build the Job with JobBuilder::build"]
pub struct JobBuilder<P> {
    script_name: &'static str,
    job_name: Option<String>,
    actor_json: Option<Value>,
    params: Option<P>,
    cron_expr: Option<String>,
    timezone: Option<String>,
    run_once_at: Option<DateTime<Utc>>,
    schedule_kind: ScheduleKind,
    enabled: bool,
    pool: Option<String>,
    region: Option<String>,
    concurrency: i32,
    timeout_ms: Option<i64>,
    retry_policy: RetryPolicy,
    misfire_policy: MisfirePolicy,
}

impl<P> JobBuilder<P>
where
    P: Serialize,
{
    /// Create a new job builder for the given script handle.
    pub fn new(handle: &ScriptHandle<P>) -> Self {
        Self {
            script_name: handle.name(),
            job_name: None,
            actor_json: None,
            params: None,
            cron_expr: None,
            timezone: None,
            run_once_at: None,
            schedule_kind: ScheduleKind::Cron,
            enabled: true,
            pool: None,
            region: None,
            concurrency: 1,
            timeout_ms: None,
            retry_policy: RetryPolicy::default(),
            misfire_policy: MisfirePolicy::default(),
        }
    }

    /// Set the job name (unique per deployment).
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.job_name = Some(name.into());
        self
    }

    /// Set opaque identity JSON persisted on [`Job::actor_json`](chronon_core::Job::actor_json).
    ///
    /// Use an identity snapshot only — do not store secrets. Unified Field hosts with Valence
    /// should prefer L1 `chronon_coordinator::JobBuilder::with_valence` instead.
    pub fn with_actor_json(mut self, actor_json: Value) -> Self {
        self.actor_json = Some(actor_json);
        self
    }

    /// Set the cron schedule.
    ///
    /// # Errors
    ///
    /// Returns [`ChrononError::InvalidCron`] when `expr` is not a valid cron expression.
    pub fn cron(mut self, expr: &str) -> Result<Self> {
        CronExpr::parse(expr, None)?;
        self.cron_expr = Some(expr.to_string());
        self.schedule_kind = ScheduleKind::Cron;
        Ok(self)
    }

    /// Set the timezone for cron evaluation.
    pub fn timezone(mut self, tz: impl Into<String>) -> Self {
        self.timezone = Some(tz.into());
        self
    }

    /// Schedule a one-time execution at the specified time.
    pub const fn run_once_at(mut self, at: DateTime<Utc>) -> Self {
        self.run_once_at = Some(at);
        self.schedule_kind = ScheduleKind::RunOnce;
        self
    }

    /// Set the job to manual-only (no automatic scheduling).
    pub const fn manual(mut self) -> Self {
        self.schedule_kind = ScheduleKind::Manual;
        self
    }

    /// Set the script parameters.
    pub fn params(mut self, params: P) -> Self {
        self.params = Some(params);
        self
    }

    /// Set the execution pool (for distributed mode).
    pub fn pool(mut self, pool: impl Into<String>) -> Self {
        self.pool = Some(pool.into());
        self
    }

    /// Set the target region (for distributed mode).
    pub fn region(mut self, region: impl Into<String>) -> Self {
        self.region = Some(region.into());
        self
    }

    /// Set maximum concurrent runs.
    pub const fn concurrency(mut self, max: i32) -> Self {
        self.concurrency = max;
        self
    }

    /// Set execution timeout in milliseconds.
    pub const fn timeout_ms(mut self, ms: i64) -> Self {
        self.timeout_ms = Some(ms);
        self
    }

    /// Set the retry policy for failed runs.
    pub const fn retry_policy(mut self, policy: RetryPolicy) -> Self {
        self.retry_policy = policy;
        self
    }

    /// Set the misfire policy for missed runs.
    pub const fn misfire_policy(mut self, policy: MisfirePolicy) -> Self {
        self.misfire_policy = policy;
        self
    }

    /// Disable the job (it won't run until enabled).
    pub const fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Build the final [`Job`] payload.
    ///
    /// # Errors
    ///
    /// - [`ChrononError::ParamError`] when [`Self::name`] was not set
    /// - [`ChrononError::InvalidCron`] / [`ChrononError::InvalidTimezone`] when cron fields are invalid
    /// - [`ChrononError::ParamError`] when params fail to serialize
    pub fn build(self) -> Result<Job> {
        let job_name = self
            .job_name
            .ok_or_else(|| ChrononError::ParamError("job name is required".to_string()))?;

        let params_json = match self.params {
            Some(p) => serde_json::to_value(&p)?,
            None => Value::Object(serde_json::Map::default()),
        };

        let actor_json = self.actor_json.unwrap_or(Value::Null);

        let cron_expr = match self.schedule_kind {
            ScheduleKind::Cron => self
                .cron_expr
                .as_deref()
                .map(|expr| CronExpr::parse(expr, self.timezone.as_deref()))
                .transpose()?,
            ScheduleKind::Manual | ScheduleKind::RunOnce => None,
        };

        let next_run_at = match self.schedule_kind {
            ScheduleKind::Cron => cron_expr.as_ref().and_then(CronExpr::next_from_now),
            ScheduleKind::RunOnce => self.run_once_at,
            ScheduleKind::Manual => None,
        };

        let mut job = Job::new(&job_name, self.script_name);
        job.enabled = self.enabled;
        job.schedule_kind = self.schedule_kind;
        job.cron_expr = cron_expr.map(|cron| cron.expression().to_string());
        job.timezone = self.timezone;
        job.run_once_at = self.run_once_at;
        job.pool = self.pool;
        job.region = self.region;
        job.actor_json = actor_json;
        job.params_json = params_json;
        job.concurrency = self.concurrency;
        job.timeout_ms = self.timeout_ms;
        job.retry_policy_json = serde_json::to_value(&self.retry_policy)?;
        job.misfire_policy_json = serde_json::to_value(&self.misfire_policy)?;
        job.next_run_at = next_run_at;
        job.current_revision = 1;

        Ok(job)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn build_cron_sets_schedule_and_next_run() {
        let handle = ScriptHandle::<()>::new("nightly_cleanup");
        let job = JobBuilder::new(&handle)
            .name("nightly-cleanup")
            .cron("0 0 * * * *")
            .expect("cron")
            .timezone("UTC")
            .build()
            .expect("build");
        assert_eq!(job.script_name, "nightly_cleanup");
        assert_eq!(job.job_name, "nightly-cleanup");
        assert_eq!(job.schedule_kind, ScheduleKind::Cron);
        assert_eq!(job.cron_expr.as_deref(), Some("0 0 * * * *"));
        assert!(job.next_run_at.is_some());
        assert!(job.actor_json.is_null());
    }

    #[test]
    fn build_run_once_sets_next_run_at() {
        let at = Utc.with_ymd_and_hms(2030, 1, 1, 0, 0, 0).unwrap();
        let handle = ScriptHandle::<()>::new("once");
        let job = JobBuilder::new(&handle)
            .name("once-job")
            .run_once_at(at)
            .build()
            .expect("build");
        assert_eq!(job.schedule_kind, ScheduleKind::RunOnce);
        assert_eq!(job.next_run_at, Some(at));
        assert!(job.cron_expr.is_none());
    }

    #[test]
    fn build_manual_clears_automatic_schedule() {
        let handle = ScriptHandle::<()>::new("manual");
        let job = JobBuilder::new(&handle)
            .name("manual-job")
            .manual()
            .build()
            .expect("build");
        assert_eq!(job.schedule_kind, ScheduleKind::Manual);
        assert!(job.next_run_at.is_none());
        assert!(job.cron_expr.is_none());
    }

    #[test]
    fn build_params_serialized() {
        #[derive(Serialize)]
        struct Params {
            n: u32,
        }
        let handle = ScriptHandle::<Params>::new("demo");
        let job = JobBuilder::new(&handle)
            .name("demo-job")
            .manual()
            .params(Params { n: 3 })
            .build()
            .expect("build");
        assert_eq!(job.params_json["n"], 3);
    }

    #[test]
    fn build_with_actor_json_round_trip() {
        let actor = serde_json::json!({ "Service": { "name": "ops" } });
        let handle = ScriptHandle::<()>::new("probe");
        let job = JobBuilder::new(&handle)
            .name("probe")
            .manual()
            .with_actor_json(actor.clone())
            .build()
            .expect("build");
        assert_eq!(job.actor_json, actor);
    }

    #[test]
    fn build_missing_name_is_param_error() {
        let handle = ScriptHandle::<()>::new("probe");
        let err = JobBuilder::new(&handle).manual().build().unwrap_err();
        match err {
            ChrononError::ParamError(msg) => assert!(msg.contains("job name")),
            other => panic!("expected ParamError, got {other}"),
        }
    }

    #[test]
    fn cron_invalid_expr_is_invalid_cron() {
        let handle = ScriptHandle::<()>::new("probe");
        let Err(err) = JobBuilder::new(&handle).cron("not-a-cron") else {
            panic!("expected InvalidCron");
        };
        assert!(matches!(err, ChrononError::InvalidCron(_)));
    }
}
