//! Revisions, scripts, leader election, partitions, and workers.

use chrono::{DateTime, Duration, Utc};

use chronon_core::error::{ChrononError, Result};
use chronon_core::models::{JobRevision, PartitionAssignment, SchedulerLeader, Script, Worker};

use super::{InMemorySchedulerStore, LEADER_ROW_ID};

pub(super) fn append_revision(
    store: &InMemorySchedulerStore,
    revision: &JobRevision,
) -> Result<()> {
    store
        .revisions
        .write()
        .entry(revision.job_id.clone())
        .or_default()
        .push(revision.clone());
    Ok(())
}

pub(super) fn list_revisions(
    store: &InMemorySchedulerStore,
    job_id: &str,
) -> Result<Vec<JobRevision>> {
    Ok(store
        .revisions
        .read()
        .get(job_id)
        .cloned()
        .unwrap_or_default())
}

pub(super) fn upsert_script(store: &InMemorySchedulerStore, script: &Script) -> Result<()> {
    store
        .scripts
        .write()
        .insert(script.script_name.clone(), script.clone());
    Ok(())
}

pub(super) fn get_script(
    store: &InMemorySchedulerStore,
    script_name: &str,
) -> Result<Option<Script>> {
    Ok(store.scripts.read().get(script_name).cloned())
}

pub(super) fn try_acquire_leader(
    store: &InMemorySchedulerStore,
    instance_id: &str,
    ttl_secs: i64,
) -> Result<bool> {
    let now = Utc::now();
    let until = now + Duration::seconds(ttl_secs);
    let mut leader = store.leader.write();
    if let Some(ref row) = *leader {
        if row.leader_lease_until > now && row.leader_instance_id != instance_id {
            return Ok(false);
        }
    }
    *leader = Some(SchedulerLeader {
        leader_id: LEADER_ROW_ID.to_string(),
        leader_instance_id: instance_id.to_string(),
        leader_lease_until: until,
        last_heartbeat_at: now,
    });
    Ok(true)
}

pub(super) fn renew_leader_lease(
    store: &InMemorySchedulerStore,
    instance_id: &str,
    ttl_secs: i64,
) -> Result<()> {
    let now = Utc::now();
    let mut leader = store.leader.write();
    let Some(ref mut row) = *leader else {
        return Ok(());
    };
    if row.leader_instance_id != instance_id {
        return Ok(());
    }
    row.leader_lease_until = now + Duration::seconds(ttl_secs);
    row.last_heartbeat_at = now;
    Ok(())
}

pub(super) fn get_leader(store: &InMemorySchedulerStore) -> Result<Option<SchedulerLeader>> {
    Ok(store.leader.read().clone())
}

pub(super) fn upsert_partition_assignment(
    store: &InMemorySchedulerStore,
    assignment: &PartitionAssignment,
) -> Result<()> {
    store
        .partitions
        .write()
        .insert(assignment.partition_id.clone(), assignment.clone());
    Ok(())
}

pub(super) fn list_partition_assignments(
    store: &InMemorySchedulerStore,
) -> Result<Vec<PartitionAssignment>> {
    Ok(store.partitions.read().values().cloned().collect())
}

pub(super) fn register_worker(store: &InMemorySchedulerStore, worker: &Worker) -> Result<()> {
    store
        .workers
        .write()
        .insert(worker.worker_id.clone(), worker.clone());
    Ok(())
}

pub(super) fn heartbeat_worker(
    store: &InMemorySchedulerStore,
    worker_id: &str,
    at: DateTime<Utc>,
) -> Result<()> {
    let mut workers = store.workers.write();
    let Some(worker) = workers.get_mut(worker_id) else {
        return Err(ChrononError::Internal(format!(
            "worker not registered: {worker_id}"
        )));
    };
    worker.last_heartbeat_at = at;
    worker.updated_at = at;
    Ok(())
}
