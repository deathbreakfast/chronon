//! Distributed run lease model.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Lease for a distributed run (exactly-one claim).
///
/// Persisted by [`SchedulerStore`](crate::store::SchedulerStore) backends that track worker
/// claims separately from the [`Run`](crate::models::Run) row. Used for reclaim and failover
/// when a worker stops renewing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lease {
    /// Unique lease row id.
    pub lease_id: String,
    /// Run this lease protects.
    pub run_id: String,
    /// Owning job, when the run was spawned from a schedule.
    pub job_id: Option<String>,
    /// Worker or coordinator instance that holds the lease.
    pub leased_by: String,
    /// Worker pool that may claim this run.
    pub pool_id: String,
    /// Lease expiry; another worker may reclaim after this instant.
    pub lease_until: DateTime<Utc>,
    /// When the lease row was first created.
    pub created_at: DateTime<Utc>,
}
