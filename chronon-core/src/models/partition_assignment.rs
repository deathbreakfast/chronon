//! Partition ownership for distributed coordinators.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Per-partition lease for coordinator shard ownership.
///
/// Each scheduler instance owns a subset of partitions; the assigner upserts rows so tick
/// discovery only scans jobs hashed into owned partitions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionAssignment {
    /// Partition index as a string key (matches hash ring slot).
    pub partition_id: String,
    /// Scheduler instance id currently responsible for this partition.
    pub owner_instance_id: String,
    /// Assignment expiry; partitions may be stolen after this instant.
    pub lease_until: DateTime<Utc>,
    /// Last time this row was written (rebalance or heartbeat).
    pub updated_at: DateTime<Utc>,
}
