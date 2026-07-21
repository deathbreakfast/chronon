//! Scheduler leader election via [`SchedulerStore`].

use std::sync::Arc;

use chrono::Utc;
use chronon_core::store::SchedulerStore;
use chronon_core::Result;

const DEFAULT_LEADER_TTL_SECS: i64 = 30;

/// Reads `CHRONON_LEADER_TTL_S` (default 30 seconds).
///
/// Lease duration passed to [`try_acquire_leader`] and [`renew_leader_lease`].
pub fn leader_ttl_secs_from_env() -> i64 {
    std::env::var("CHRONON_LEADER_TTL_S")
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|&n: &i64| n >= 1)
        .unwrap_or(DEFAULT_LEADER_TTL_SECS)
}

/// Attempts to become the cluster leader, returning `true` on success.
///
/// Called at coordinator boot before partition assignment in distributed mode.
pub async fn try_acquire_leader(
    store: &Arc<dyn SchedulerStore>,
    instance_id: &str,
) -> Result<bool> {
    store
        .try_acquire_leader(instance_id, leader_ttl_secs_from_env())
        .await
}

/// Renews the leader lease for `instance_id` if it currently holds leadership.
///
/// Fails when another instance has taken over or the lease expired.
pub async fn renew_leader_lease(store: &Arc<dyn SchedulerStore>, instance_id: &str) -> Result<()> {
    store
        .renew_leader_lease(instance_id, leader_ttl_secs_from_env())
        .await
}

/// Returns the current leader instance id and lease expiry, if any row exists.
pub async fn current_leader(
    store: &Arc<dyn SchedulerStore>,
) -> Result<Option<(String, chrono::DateTime<Utc>)>> {
    Ok(store
        .get_leader()
        .await?
        .map(|l| (l.leader_instance_id, l.leader_lease_until)))
}

/// Returns `true` when `instance_id` holds a non-expired leader lease.
pub async fn am_i_leader(store: &Arc<dyn SchedulerStore>, instance_id: &str) -> Result<bool> {
    let Some((id, until)) = current_leader(store).await? else {
        return Ok(false);
    };
    Ok(until > Utc::now() && id == instance_id)
}
