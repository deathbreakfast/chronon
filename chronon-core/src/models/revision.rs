//! Job revision model - represents a snapshot of job configuration changes.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A revision of a job's configuration.
///
/// Every configuration change creates a new revision for audit purposes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobRevision {
    /// Unique identifier (UUID).
    pub revision_id: String,

    /// Job this revision belongs to.
    pub job_id: String,

    /// Sequential revision number (1, 2, 3...).
    pub revision_number: i32,

    /// When this revision was created.
    pub changed_at: DateTime<Utc>,

    /// Who made this change (serialized Actor).
    pub changed_by_actor_json: Value,

    /// Full snapshot of the job configuration at this revision.
    pub snapshot_json: Value,
}

impl JobRevision {
    /// Create a new revision for a job.
    pub fn new(
        job_id: impl Into<String>,
        revision_number: i32,
        changed_by_actor_json: Value,
        snapshot_json: Value,
    ) -> Self {
        Self {
            revision_id: uuid::Uuid::new_v4().to_string(),
            job_id: job_id.into(),
            revision_number,
            changed_at: Utc::now(),
            changed_by_actor_json,
            snapshot_json,
        }
    }
}
