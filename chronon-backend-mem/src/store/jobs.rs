//! Job CRUD helpers for the in-memory store.

use chrono::{DateTime, Utc};

use chronon_core::models::Job;
use chronon_core::Result;

use super::InMemorySchedulerStore;

pub(super) fn upsert_job(store: &InMemorySchedulerStore, job: &Job) -> Result<()> {
    store.write_job(job.clone())
}

pub(super) fn get_job(store: &InMemorySchedulerStore, job_id: &str) -> Result<Option<Job>> {
    Ok(store.jobs.read().expect("jobs lock").get(job_id).cloned())
}

pub(super) async fn get_job_by_name(
    store: &InMemorySchedulerStore,
    job_name: &str,
) -> Result<Option<Job>> {
    let id = store
        .jobs_by_name
        .read()
        .expect("jobs_by_name lock")
        .get(job_name)
        .cloned();
    match id {
        Some(job_id) => get_job(store, &job_id),
        None => Ok(None),
    }
}

pub(super) fn list_jobs(store: &InMemorySchedulerStore) -> Result<Vec<Job>> {
    Ok(store
        .jobs
        .read()
        .expect("jobs lock")
        .values()
        .cloned()
        .collect())
}

pub(super) fn list_due_jobs(
    store: &InMemorySchedulerStore,
    before: DateTime<Utc>,
) -> Result<Vec<Job>> {
    Ok(store
        .jobs
        .read()
        .expect("jobs lock")
        .values()
        .filter(|j| j.enabled && j.next_run_at.is_some_and(|t| t <= before))
        .cloned()
        .collect())
}

pub(super) fn pause_job(store: &InMemorySchedulerStore, job_id: &str) -> Result<()> {
    store.mutate_job(job_id, |j| j.enabled = false)
}

pub(super) fn resume_job(store: &InMemorySchedulerStore, job_id: &str) -> Result<()> {
    store.mutate_job(job_id, |j| j.enabled = true)
}
