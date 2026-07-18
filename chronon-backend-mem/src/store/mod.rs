//! In-memory [`SchedulerStore`] implementation split by concern.

mod claims;
mod coordinator;
mod jobs;
mod runs;
mod trait_impl;

use std::collections::HashMap;
use std::sync::RwLock;

use chrono::{DateTime, Utc};
use chronon_core::error::{ChrononError, Result};
use chronon_core::models::{
    Job, JobRevision, PartitionAssignment, Run, SchedulerLeader, Script, Worker,
};

/// Fixed primary key for the singleton leader election row.
pub const LEADER_ROW_ID: &str = "singleton";

/// Thread-safe in-memory persistence for jobs, runs, and coordinator metadata.
///
/// Process-local and **non-durable** — suitable for Mode 1 embedded experiments, unit tests,
/// examples, and benchmarks. Do **not** share across processes; use SQLite or Postgres for
/// durable / Mode 2 topologies.
///
/// Enable the facade `mem` feature to re-export this type from `chronon`. Wire with
/// `ChrononBuilder::scheduler_store(Arc::new(InMemorySchedulerStore::new()))` or
/// [`crate::install_default_mem_store`].
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use chronon_backend_mem::InMemorySchedulerStore;
/// use chronon_core::{Job, SchedulerStore};
///
/// # #[tokio::main]
/// # async fn main() -> chronon_core::Result<()> {
/// let store = Arc::new(InMemorySchedulerStore::new());
/// store.upsert_job(&Job::new("demo", "noop")).await?;
/// assert_eq!(store.list_jobs().await?.len(), 1);
/// # Ok(())
/// # }
/// ```
#[derive(Default)]
pub struct InMemorySchedulerStore {
    /// Jobs keyed by [`Job::job_id`].
    pub(super) jobs: RwLock<HashMap<String, Job>>,
    /// Secondary index from [`Job::job_name`] to job id.
    pub(super) jobs_by_name: RwLock<HashMap<String, String>>,
    /// Runs keyed by [`Run::run_id`].
    pub(super) runs: RwLock<HashMap<String, Run>>,
    /// Job revision history keyed by job id.
    pub(super) revisions: RwLock<HashMap<String, Vec<JobRevision>>>,
    /// Script metadata keyed by script name.
    pub(super) scripts: RwLock<HashMap<String, Script>>,
    /// Singleton scheduler leader row.
    pub(super) leader: RwLock<Option<SchedulerLeader>>,
    /// Partition assignments keyed by partition id.
    pub(super) partitions: RwLock<HashMap<String, PartitionAssignment>>,
    /// Registered workers keyed by worker id.
    pub(super) workers: RwLock<HashMap<String, Worker>>,
}

impl InMemorySchedulerStore {
    /// Creates an empty store.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[tokio::main]
    /// # async fn main() {
    /// use chronon_backend_mem::InMemorySchedulerStore;
    /// use chronon_core::SchedulerStore;
    ///
    /// let store = InMemorySchedulerStore::new();
    /// assert!(store.list_jobs().await.unwrap().is_empty());
    /// # }
    /// ```
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert or replace a job and update the name index.
    pub(super) fn write_job(&self, job: Job) -> Result<()> {
        let mut jobs = self.jobs.write().expect("jobs lock");
        let mut by_name = self.jobs_by_name.write().expect("jobs_by_name lock");
        by_name.insert(job.job_name.clone(), job.job_id.clone());
        jobs.insert(job.job_id.clone(), job);
        Ok(())
    }

    /// Mutate an existing job and stamp `updated_at` to now.
    pub(super) fn mutate_job<F>(&self, job_id: &str, f: F) -> Result<()>
    where
        F: FnOnce(&mut Job),
    {
        let mut jobs = self.jobs.write().expect("jobs lock");
        let job = jobs
            .get_mut(job_id)
            .ok_or_else(|| ChrononError::JobNotFound(job_id.to_string()))?;
        f(job);
        job.updated_at = Utc::now();
        Ok(())
    }

    /// Mutate an existing job and stamp `updated_at` to `now`.
    pub(super) fn mutate_job_at<F>(
        &self,
        job_id: &str,
        now: DateTime<Utc>,
        f: F,
    ) -> Result<()>
    where
        F: FnOnce(&mut Job),
    {
        let mut jobs = self.jobs.write().expect("jobs lock");
        let job = jobs
            .get_mut(job_id)
            .ok_or_else(|| ChrononError::JobNotFound(job_id.to_string()))?;
        f(job);
        job.updated_at = now;
        Ok(())
    }
}
