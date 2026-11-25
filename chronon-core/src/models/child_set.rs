//! Child set model - one batch of child runs spawned by a parent run.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// One batch of child runs spawned by a parent run, with its settle counters.
///
/// The parent run writes the set and every child [`Run`](crate::models::Run) in one store
/// transaction ([`SchedulerStore::create_child_set_with_runs`](crate::SchedulerStore::create_child_set_with_runs)).
/// Each finished child bumps one counter through
/// [`SchedulerStore::record_child_result`](crate::SchedulerStore::record_child_result); the set
/// is settled once `succeeded + failed + skipped == count`.
///
/// # Examples
///
/// ```
/// use chronon_core::ChildSet;
///
/// let set = ChildSet::new("parent-run", "child-job", 3);
/// assert_eq!(set.count, 3);
/// assert_eq!(set.finished(), 0);
/// assert!(set.settled_at.is_none());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChildSet {
    /// Unique identifier (UUID).
    pub set_id: String,
    /// Run that spawned the set.
    pub parent_run_id: String,
    /// Job whose runs make up the set.
    pub child_job_id: String,
    /// Number of child runs in the set.
    pub count: u32,
    /// Children that finished successfully.
    pub succeeded: u32,
    /// Children that failed after their last attempt.
    pub failed: u32,
    /// Children that never ran (skipped or canceled).
    pub skipped: u32,
    /// When every child had finished.
    pub settled_at: Option<DateTime<Utc>>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
}

impl ChildSet {
    /// New unsettled set with a generated `set_id` and zeroed counters.
    pub fn new(
        parent_run_id: impl Into<String>,
        child_job_id: impl Into<String>,
        count: u32,
    ) -> Self {
        Self {
            set_id: uuid::Uuid::new_v4().to_string(),
            parent_run_id: parent_run_id.into(),
            child_job_id: child_job_id.into(),
            count,
            succeeded: 0,
            failed: 0,
            skipped: 0,
            settled_at: None,
            created_at: Utc::now(),
        }
    }

    /// Children that have reported a result so far.
    pub const fn finished(&self) -> u32 {
        self.succeeded
            .saturating_add(self.failed)
            .saturating_add(self.skipped)
    }
}

/// Result of one child run, recorded on its [`ChildSet`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChildOutcome {
    /// The child finished successfully.
    Succeeded,
    /// The child failed after its last attempt.
    Failed,
    /// The child never ran.
    Skipped,
}
