//! Run-once and tick claim helpers.

use chrono::{DateTime, Duration, Utc};

use chronon_core::models::ScheduleKind;
use chronon_core::Result;

use super::InMemorySchedulerStore;

pub(super) fn try_claim_run_once(
    store: &InMemorySchedulerStore,
    job_id: &str,
    claimed_by: &str,
    now: DateTime<Utc>,
    claim_ttl_secs: i64,
) -> Result<bool> {
    let mut jobs = store.jobs.write().expect("jobs lock");
    let Some(job) = jobs.get_mut(job_id) else {
        return Ok(false);
    };
    if job.run_once_completed_at.is_some() {
        return Ok(false);
    }
    if job.run_once_claimed_at.is_some()
        && job.run_once_claim_expires_at.is_some_and(|e| e > now)
        && job.run_once_claimed_by.as_deref() != Some(claimed_by)
    {
        return Ok(false);
    }
    job.run_once_claimed_at = Some(now);
    job.run_once_claimed_by = Some(claimed_by.to_string());
    job.run_once_claim_expires_at = Some(now + Duration::seconds(claim_ttl_secs));
    job.updated_at = now;
    Ok(true)
}

pub(super) fn mark_run_once_completed(
    store: &InMemorySchedulerStore,
    job_id: &str,
    completed_at: DateTime<Utc>,
) -> Result<()> {
    store.mutate_job(job_id, |j| {
        j.run_once_completed_at = Some(completed_at);
        j.run_once_claimed_at = None;
        j.run_once_claimed_by = None;
        j.run_once_claim_expires_at = None;
    })
}

pub(super) fn release_run_once_claim(
    store: &InMemorySchedulerStore,
    job_id: &str,
    claimed_by: &str,
    now: DateTime<Utc>,
) -> Result<()> {
    store.mutate_job_at(job_id, now, |j| {
        if j.run_once_claimed_by.as_deref() == Some(claimed_by) {
            j.run_once_claimed_at = None;
            j.run_once_claimed_by = None;
            j.run_once_claim_expires_at = None;
        }
    })
}

pub(super) fn find_due_job_ids_in_partitions(
    store: &InMemorySchedulerStore,
    owned_partitions: &[u32],
    due_until: DateTime<Utc>,
    limit: u32,
) -> Result<Vec<String>> {
    if owned_partitions.is_empty() || limit == 0 {
        return Ok(vec![]);
    }
    let parts: std::collections::HashSet<i64> =
        owned_partitions.iter().map(|&p| i64::from(p)).collect();
    let now = Utc::now();
    let mut due: Vec<_> = store
        .jobs
        .read()
        .expect("jobs lock")
        .values()
        .filter(|j| j.enabled && j.schedule_kind != ScheduleKind::Manual)
        .filter(|j| j.next_run_at.is_some_and(|t| t <= due_until))
        .filter(|j| j.partition_hash.is_some_and(|h| parts.contains(&h)))
        .filter(|j| j.claim_lease_until.is_none_or(|u| u < now))
        .filter(|j| j.schedule_kind != ScheduleKind::RunOnce || j.run_once_completed_at.is_none())
        .cloned()
        .collect();
    due.sort_by_key(|j| j.next_run_at.unwrap_or(due_until));
    Ok(due
        .into_iter()
        .take(limit as usize)
        .map(|j| j.job_id)
        .collect())
}

pub(super) fn min_next_run_at_in_partitions(
    store: &InMemorySchedulerStore,
    owned_partitions: &[u32],
) -> Result<Option<DateTime<Utc>>> {
    if owned_partitions.is_empty() {
        return Ok(None);
    }
    let parts: std::collections::HashSet<i64> =
        owned_partitions.iter().map(|&p| i64::from(p)).collect();
    Ok(store
        .jobs
        .read()
        .expect("jobs lock")
        .values()
        .filter(|j| j.enabled && j.schedule_kind != ScheduleKind::Manual)
        .filter(|j| j.partition_hash.is_some_and(|h| parts.contains(&h)))
        .filter_map(|j| j.next_run_at)
        .min())
}

pub(super) fn claim_job_for_tick(
    store: &InMemorySchedulerStore,
    job_id: &str,
    claim_id: &str,
    now: DateTime<Utc>,
    lease_ttl_secs: i64,
) -> Result<bool> {
    let mut jobs = store.jobs.write().expect("jobs lock");
    let Some(job) = jobs.get_mut(job_id) else {
        return Ok(false);
    };
    if !job.enabled {
        return Ok(false);
    }
    if job.claim_lease_until.is_some_and(|u| u >= now) {
        return Ok(false);
    }
    job.claim_lease_id = Some(claim_id.to_string());
    job.claim_lease_until = Some(now + Duration::seconds(lease_ttl_secs));
    job.updated_at = now;
    Ok(true)
}

pub(super) fn release_job_tick_claim(
    store: &InMemorySchedulerStore,
    job_id: &str,
) -> Result<()> {
    store.mutate_job(job_id, |j| {
        j.claim_lease_id = None;
        j.claim_lease_until = None;
    })
}

pub(super) fn persist_post_tick_job_state(
    store: &InMemorySchedulerStore,
    job_id: &str,
    next_run_at: Option<DateTime<Utc>>,
) -> Result<()> {
    store.mutate_job(job_id, |j| {
        j.next_run_at = next_run_at;
        j.claim_lease_id = None;
        j.claim_lease_until = None;
    })
}
