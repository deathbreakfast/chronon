//! Scheduler leader election singleton row.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Singleton row for distributed scheduler leader election.
///
/// Only the leader instance runs the coordinator tick loop in split deployments. Other
/// instances renew partition assignments and execute runs as workers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerLeader {
    /// Fixed row id (typically `"singleton"`).
    pub leader_id: String,
    /// Instance id of the current leader.
    pub leader_instance_id: String,
    /// Leader lease expiry; followers may campaign after this instant.
    pub leader_lease_until: DateTime<Utc>,
    /// Last successful leader heartbeat (diagnostics and staleness checks).
    pub last_heartbeat_at: DateTime<Utc>,
}
