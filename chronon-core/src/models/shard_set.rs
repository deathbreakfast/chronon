//! Shard set model - a cursor walked in ranges by a sharded job.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A sharded job's walk over a cursor field, one range (shard) per run.
///
/// `next_start` is where the next shard begins. Writers move it with
/// [`SchedulerStore::update_shard_set`](crate::SchedulerStore::update_shard_set), which only
/// succeeds when the stored value still equals the one they read. Shards record progress in
/// `checkpoints_json`, an object keyed by shard index.
///
/// # Examples
///
/// ```
/// use chronon_core::ShardSet;
/// use serde_json::json;
///
/// let set = ShardSet::new("parent-run", "job-1", "id", json!(0), 1_000);
/// assert_eq!(set.next_start, json!(0));
/// assert_eq!(set.checkpoints_json, json!({}));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShardSet {
    /// Unique identifier (UUID).
    pub set_id: String,
    /// Run that started the walk.
    pub parent_run_id: String,
    /// Sharded job.
    pub job_id: String,
    /// Field the cursor walks (for example `id`).
    pub cursor_field: String,
    /// Start of the next shard to hand out.
    pub next_start: Value,
    /// Where the walk stops; `None` walks until a shard comes back short.
    pub end: Option<Value>,
    /// Rows each shard covers.
    pub rows_per_shard: u32,
    /// Whether handing out new shards is paused.
    pub paused: bool,
    /// Last checkpoint per shard, keyed by shard index.
    pub checkpoints_json: Value,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
}

impl ShardSet {
    /// New unpaused set with a generated `set_id` and no checkpoints.
    pub fn new(
        parent_run_id: impl Into<String>,
        job_id: impl Into<String>,
        cursor_field: impl Into<String>,
        start: Value,
        rows_per_shard: u32,
    ) -> Self {
        Self {
            set_id: uuid::Uuid::new_v4().to_string(),
            parent_run_id: parent_run_id.into(),
            job_id: job_id.into(),
            cursor_field: cursor_field.into(),
            next_start: start,
            end: None,
            rows_per_shard,
            paused: false,
            checkpoints_json: Value::Object(serde_json::Map::default()),
            created_at: Utc::now(),
        }
    }
}
