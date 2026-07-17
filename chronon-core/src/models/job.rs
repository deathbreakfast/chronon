//! Job model - represents a scheduled job configuration.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// How the job is scheduled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ScheduleKind {
    /// Recurring cron schedule.
    #[default]
    Cron,
    /// One-time execution at a specific time.
    RunOnce,
    /// Manual execution only (no automatic scheduling).
    Manual,
}

/// Retry policy for failed or timed-out runs.
///
/// `max_attempts` is the number of **additional** retries after the first attempt
/// (attempt 1). `0` means no retries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    /// Maximum number of retry attempts after the first execution.
    pub max_attempts: u32,

    /// Base delay between retries in milliseconds.
    pub base_delay_ms: u64,

    /// Exponential backoff multiplier (`1.0` = no backoff).
    pub backoff_multiplier: f64,

    /// Cap on delay in milliseconds; `0` means uncapped.
    pub max_delay_ms: u64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 0,
            base_delay_ms: 0,
            backoff_multiplier: 1.0,
            max_delay_ms: 0,
        }
    }
}

impl RetryPolicy {
    /// Whether a failed attempt with this `attempt` number should schedule another run.
    pub fn should_retry(&self, attempt: i32) -> bool {
        attempt > 0 && (attempt as u32) <= self.max_attempts
    }

    /// Delay before the next attempt after `failed_attempt` finishes.
    pub fn delay_ms_after(&self, failed_attempt: i32) -> u64 {
        let exp = failed_attempt.saturating_sub(1).max(0);
        let raw = (self.base_delay_ms as f64) * self.backoff_multiplier.powi(exp);
        let ms = if raw.is_finite() && raw > 0.0 {
            raw.min(u64::MAX as f64) as u64
        } else {
            0
        };
        if self.max_delay_ms == 0 {
            ms
        } else {
            ms.min(self.max_delay_ms)
        }
    }
}

/// Policy for handling missed scheduled fires at tick time.
///
/// When `max_misfire_window_secs == 0`, misfire gating is disabled (legacy: always enqueue).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MisfirePolicy {
    /// When true and within the misfire window, enqueue one coalesced run for the miss.
    pub run_immediately: bool,

    /// Max lateness (seconds) that still qualifies for misfire recovery; `0` disables gating.
    pub max_misfire_window_secs: u64,
}

/// A scheduled job configuration.
///
/// Jobs reference a script and define when/how it should run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    /// Unique identifier (UUID).
    pub job_id: String,

    /// Human-readable name, unique in the deployment.
    pub job_name: String,

    /// Name of the script to execute.
    pub script_name: String,

    /// Signature hash at job creation (for validation).
    pub script_sig_hash: String,

    /// Whether the job is enabled.
    pub enabled: bool,

    /// How the job is scheduled.
    pub schedule_kind: ScheduleKind,

    /// Cron expression (when schedule_kind is Cron).
    pub cron_expr: Option<String>,

    /// Timezone for cron evaluation (e.g., "America/New_York").
    pub timezone: Option<String>,

    /// One-time execution timestamp (when schedule_kind is RunOnce).
    pub run_once_at: Option<DateTime<Utc>>,

    /// When a coordinator claimed this run-once job for enqueue (distributed safety).
    pub run_once_claimed_at: Option<DateTime<Utc>>,

    /// Coordinator instance id that holds the claim (`coordinator_instance_id`).
    pub run_once_claimed_by: Option<String>,

    /// Set after a scheduled run-once execution is successfully enqueued (persisted run row).
    pub run_once_completed_at: Option<DateTime<Utc>>,

    /// Claim lease expiry; after this, another coordinator may reclaim if not completed.
    pub run_once_claim_expires_at: Option<DateTime<Utc>>,

    /// Partition hash for coordinator sharding (distributed mode).
    pub partition_hash: Option<i64>,

    /// Coordinator tick-claim holder id.
    pub claim_lease_id: Option<String>,

    /// Coordinator tick-claim lease expiry.
    pub claim_lease_until: Option<DateTime<Utc>>,

    // Distributed-mode fields (stored but ignored in local mode)
    /// Execution pool (e.g., "global", "region/us-west").
    pub pool: Option<String>,

    /// Target region for execution.
    pub region: Option<String>,

    /// Additional placement constraints as JSON.
    pub placement_json: Option<Value>,

    // Identity for permission reconstruction
    /// Serialized Actor for identity reconstruction.
    pub actor_json: Value,

    /// Parameters to pass to the script.
    pub params_json: Value,

    /// Maximum concurrent runs allowed.
    pub concurrency: i32,

    /// Execution timeout in milliseconds.
    pub timeout_ms: Option<i64>,

    /// Retry policy JSON ([`RetryPolicy`]).
    pub retry_policy_json: Value,

    /// Misfire policy JSON ([`MisfirePolicy`]).
    pub misfire_policy_json: Value,

    /// Limits for parent/child runs.
    pub parent_limits_json: Option<Value>,

    /// When the job should next run.
    pub next_run_at: Option<DateTime<Utc>>,

    /// Current revision number.
    pub current_revision: i32,

    /// Last modification timestamp.
    pub updated_at: DateTime<Utc>,

    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
}

impl Job {
    /// Create a baseline job record with generated IDs and defaults.
    ///
    /// This constructor intentionally leaves identity and advanced scheduling
    /// fields in default/empty form. Populate cron, actor, and params fields
    /// before persistence (via coordinator service, HTTP upsert API, or direct store calls).
    pub fn new(job_name: impl Into<String>, script_name: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            job_id: uuid::Uuid::new_v4().to_string(),
            job_name: job_name.into(),
            script_name: script_name.into(),
            script_sig_hash: String::new(),
            enabled: true,
            schedule_kind: ScheduleKind::default(),
            cron_expr: None,
            timezone: None,
            run_once_at: None,
            run_once_claimed_at: None,
            run_once_claimed_by: None,
            run_once_completed_at: None,
            run_once_claim_expires_at: None,
            partition_hash: None,
            claim_lease_id: None,
            claim_lease_until: None,
            pool: None,
            region: None,
            placement_json: None,
            actor_json: Value::Null,
            params_json: Value::Object(serde_json::Map::default()),
            concurrency: 1,
            timeout_ms: None,
            retry_policy_json: serde_json::to_value(RetryPolicy::default()).unwrap_or_default(),
            misfire_policy_json: serde_json::to_value(MisfirePolicy::default()).unwrap_or_default(),
            parent_limits_json: None,
            next_run_at: None,
            current_revision: 1,
            updated_at: now,
            created_at: now,
        }
    }

    /// Decode [`RetryPolicy`] from [`Self::retry_policy_json`], or default on null/invalid.
    pub fn retry_policy(&self) -> RetryPolicy {
        serde_json::from_value(self.retry_policy_json.clone()).unwrap_or_default()
    }

    /// Decode [`MisfirePolicy`] from [`Self::misfire_policy_json`], or default on null/invalid.
    pub fn misfire_policy(&self) -> MisfirePolicy {
        serde_json::from_value(self.misfire_policy_json.clone()).unwrap_or_default()
    }

    /// Persist a typed retry policy into [`Self::retry_policy_json`].
    pub fn set_retry_policy(&mut self, policy: &RetryPolicy) {
        self.retry_policy_json = serde_json::to_value(policy).unwrap_or_default();
    }

    /// Persist a typed misfire policy into [`Self::misfire_policy_json`].
    pub fn set_misfire_policy(&mut self, policy: &MisfirePolicy) {
        self.misfire_policy_json = serde_json::to_value(policy).unwrap_or_default();
    }
}

#[cfg(test)]
mod policy_tests {
    use super::*;

    #[test]
    fn retry_default_does_not_retry() {
        let p = RetryPolicy::default();
        assert!(!p.should_retry(1));
        assert_eq!(p.delay_ms_after(1), 0);
    }

    #[test]
    fn retry_backoff_and_cap() {
        let p = RetryPolicy {
            max_attempts: 3,
            base_delay_ms: 100,
            backoff_multiplier: 2.0,
            max_delay_ms: 250,
        };
        assert!(p.should_retry(1));
        assert!(p.should_retry(3));
        assert!(!p.should_retry(4));
        assert_eq!(p.delay_ms_after(1), 100);
        assert_eq!(p.delay_ms_after(2), 200);
        assert_eq!(p.delay_ms_after(3), 250);
    }

    #[test]
    fn job_policy_roundtrip() {
        let mut job = Job::new("n", "s");
        let retry = RetryPolicy {
            max_attempts: 2,
            base_delay_ms: 50,
            backoff_multiplier: 1.5,
            max_delay_ms: 500,
        };
        let misfire = MisfirePolicy {
            run_immediately: true,
            max_misfire_window_secs: 3600,
        };
        job.set_retry_policy(&retry);
        job.set_misfire_policy(&misfire);
        assert_eq!(job.retry_policy().max_attempts, 2);
        assert!(job.misfire_policy().run_immediately);
        assert_eq!(job.misfire_policy().max_misfire_window_secs, 3600);
    }
}
