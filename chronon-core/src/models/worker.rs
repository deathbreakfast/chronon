//! Registered worker instances.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Worker registration status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum WorkerStatus {
    /// Accepting new run claims.
    #[default]
    Online,
    /// Finishing in-flight runs but not claiming new work.
    Draining,
    /// Not heartbeating; excluded from placement.
    Offline,
}

/// Registered Chronon worker instance (heartbeat).
///
/// Workers register at startup and send periodic heartbeats so coordinators can detect
/// stale capacity and route claims to live pools.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Worker {
    /// Stable worker instance id.
    pub worker_id: String,
    /// Execution pool this worker serves (matches [`Run::pool_id`](crate::models::Run::pool_id)).
    pub pool_id: String,
    /// Optional placement cell / availability zone label.
    pub cell_id: Option<String>,
    /// Whether the worker accepts new claims.
    pub status: WorkerStatus,
    /// Last heartbeat timestamp (updated by [`SchedulerStore::heartbeat_worker`](crate::store::SchedulerStore::heartbeat_worker)).
    pub last_heartbeat_at: DateTime<Utc>,
    /// Opaque capacity hints (concurrency limits, CPU, etc.).
    pub capacity_json: Option<Value>,
    /// Row creation time.
    pub created_at: DateTime<Utc>,
    /// Last metadata update.
    pub updated_at: DateTime<Utc>,
}
