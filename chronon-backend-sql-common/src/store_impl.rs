//! [`SchedulerStore`](chronon_core::store::SchedulerStore) trait surface for [`SqlSchedulerStore`].
//!
//! Internal — delegates to [`jobs`](crate::jobs), [`runs`](crate::runs), [`claims`](crate::claims),
//! and [`coordinator`](crate::coordinator) modules.

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use chronon_core::models::{
    ChildOutcome, ChildSet, Job, JobRevision, PartitionAssignment, Run, RunKey, RunStatus,
    SchedulerLeader, Script, ShardSet, Worker,
};
use chronon_core::store::SchedulerStore;
use chronon_core::Result;

use serde_json::Value;

use crate::{claims, coordinator, jobs, runs, sets, SqlSchedulerStore};

#[async_trait]
impl SchedulerStore for SqlSchedulerStore {
    async fn upsert_job(&self, job: &Job) -> Result<()> {
        jobs::upsert_job(self, job).await
    }

    async fn get_job(&self, job_id: &str) -> Result<Option<Job>> {
        jobs::get_job(self, job_id).await
    }

    async fn get_job_by_name(&self, job_name: &str) -> Result<Option<Job>> {
        jobs::get_job_by_name(self, job_name).await
    }

    async fn list_jobs(&self) -> Result<Vec<Job>> {
        jobs::list_jobs(self).await
    }

    async fn list_due_jobs(&self, before: DateTime<Utc>) -> Result<Vec<Job>> {
        jobs::list_due_jobs(self, before).await
    }

    async fn pause_job(&self, job_id: &str) -> Result<()> {
        jobs::pause_job(self, job_id).await
    }

    async fn resume_job(&self, job_id: &str) -> Result<()> {
        jobs::resume_job(self, job_id).await
    }

    async fn delete_job(&self, job_id: &str) -> Result<bool> {
        jobs::delete_job(self, job_id).await
    }

    async fn create_run(&self, run: &Run) -> Result<()> {
        runs::create_run(self, run).await
    }

    async fn update_run(&self, run: &Run) -> Result<()> {
        runs::update_run(self, run).await
    }

    async fn get_run(&self, run_id: &str) -> Result<Option<Run>> {
        runs::get_run(self, run_id).await
    }

    async fn list_runs_for_job(&self, job_id: &str, limit: usize) -> Result<Vec<Run>> {
        runs::list_runs_for_job(self, job_id, limit).await
    }

    async fn list_runs_filtered(
        &self,
        job_id: Option<&str>,
        status: Option<RunStatus>,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<Run>> {
        runs::list_runs_filtered(self, job_id, status, offset, limit).await
    }

    async fn list_runs_after(
        &self,
        job_id: Option<&str>,
        status: Option<RunStatus>,
        after: Option<&RunKey>,
        limit: usize,
    ) -> Result<Vec<Run>> {
        runs::list_runs_after(self, job_id, status, after, limit).await
    }

    async fn claim_next_queued(
        &self,
        pool_id: &str,
        worker_id: &str,
        now: DateTime<Utc>,
        lease_ttl_secs: i64,
    ) -> Result<Option<Run>> {
        runs::claim_next_queued(self, pool_id, worker_id, now, lease_ttl_secs).await
    }

    async fn claim_run_by_id(
        &self,
        run_id: &str,
        pool_id: &str,
        worker_id: &str,
        now: DateTime<Utc>,
        lease_ttl_secs: i64,
    ) -> Result<Option<Run>> {
        runs::claim_run_by_id(self, run_id, pool_id, worker_id, now, lease_ttl_secs).await
    }

    async fn claim_runs_by_ids(
        &self,
        run_ids: &[&str],
        pool_id: &str,
        worker_id: &str,
        now: DateTime<Utc>,
        lease_ttl_secs: i64,
    ) -> Result<Vec<Run>> {
        runs::claim_runs_by_ids(self, run_ids, pool_id, worker_id, now, lease_ttl_secs).await
    }

    async fn renew_run_lease(
        &self,
        run_id: &str,
        worker_id: &str,
        now: DateTime<Utc>,
        lease_ttl_secs: i64,
    ) -> Result<bool> {
        runs::renew_run_lease(self, run_id, worker_id, now, lease_ttl_secs).await
    }

    async fn reclaim_expired_run_leases(&self, now: DateTime<Utc>) -> Result<Vec<String>> {
        runs::reclaim_expired_run_leases(self, now).await
    }

    async fn set_run_outputs(&self, run_id: &str, outputs: &Value) -> Result<()> {
        sets::set_run_outputs(self, run_id, outputs).await
    }

    async fn count_active_runs_for_job(&self, job_id: &str) -> Result<i64> {
        sets::count_active_runs_for_job(self, job_id).await
    }

    async fn create_child_set_with_runs(&self, set: &ChildSet, runs: &[Run]) -> Result<()> {
        sets::create_child_set_with_runs(self, set, runs).await
    }

    async fn get_child_set(&self, set_id: &str) -> Result<Option<ChildSet>> {
        sets::get_child_set(self, set_id).await
    }

    async fn list_child_sets_for_run(&self, parent_run_id: &str) -> Result<Vec<ChildSet>> {
        sets::list_child_sets_for_run(self, parent_run_id).await
    }

    async fn record_child_result(&self, set_id: &str, outcome: ChildOutcome) -> Result<ChildSet> {
        sets::record_child_result(self, set_id, outcome).await
    }

    async fn settle_child_set(&self, set_id: &str, at: DateTime<Utc>) -> Result<()> {
        sets::settle_child_set(self, set_id, at).await
    }

    async fn create_shard_set(&self, set: &ShardSet) -> Result<()> {
        sets::create_shard_set(self, set).await
    }

    async fn get_shard_set(&self, set_id: &str) -> Result<Option<ShardSet>> {
        sets::get_shard_set(self, set_id).await
    }

    async fn update_shard_set(&self, set: &ShardSet, expected_next_start: &Value) -> Result<bool> {
        sets::update_shard_set(self, set, expected_next_start).await
    }

    async fn set_shard_checkpoint(
        &self,
        set_id: &str,
        shard_index: u32,
        value: &Value,
    ) -> Result<()> {
        sets::set_shard_checkpoint(self, set_id, shard_index, value).await
    }

    async fn append_revision(&self, revision: &JobRevision) -> Result<()> {
        coordinator::append_revision(self, revision).await
    }

    async fn list_revisions(&self, job_id: &str) -> Result<Vec<JobRevision>> {
        coordinator::list_revisions(self, job_id).await
    }

    async fn upsert_script(&self, script: &Script) -> Result<()> {
        coordinator::upsert_script(self, script).await
    }

    async fn get_script(&self, script_name: &str) -> Result<Option<Script>> {
        coordinator::get_script(self, script_name).await
    }

    async fn try_claim_run_once(
        &self,
        job_id: &str,
        claimed_by: &str,
        now: DateTime<Utc>,
        claim_ttl_secs: i64,
    ) -> Result<bool> {
        claims::try_claim_run_once(self, job_id, claimed_by, now, claim_ttl_secs).await
    }

    async fn mark_run_once_completed(
        &self,
        job_id: &str,
        completed_at: DateTime<Utc>,
    ) -> Result<()> {
        claims::mark_run_once_completed(self, job_id, completed_at).await
    }

    async fn release_run_once_claim(
        &self,
        job_id: &str,
        claimed_by: &str,
        now: DateTime<Utc>,
    ) -> Result<()> {
        claims::release_run_once_claim(self, job_id, claimed_by, now).await
    }

    async fn find_due_job_ids_in_partitions(
        &self,
        owned_partitions: &[u32],
        due_until: DateTime<Utc>,
        limit: u32,
    ) -> Result<Vec<String>> {
        claims::find_due_job_ids_in_partitions(self, owned_partitions, due_until, limit).await
    }

    async fn min_next_run_at_in_partitions(
        &self,
        owned_partitions: &[u32],
    ) -> Result<Option<DateTime<Utc>>> {
        claims::min_next_run_at_in_partitions(self, owned_partitions).await
    }

    async fn claim_job_for_tick(
        &self,
        job_id: &str,
        claim_id: &str,
        now: DateTime<Utc>,
        lease_ttl_secs: i64,
    ) -> Result<bool> {
        claims::claim_job_for_tick(self, job_id, claim_id, now, lease_ttl_secs).await
    }

    async fn release_job_tick_claim(&self, job_id: &str) -> Result<()> {
        claims::release_job_tick_claim(self, job_id).await
    }

    async fn persist_post_tick_job_state(
        &self,
        job_id: &str,
        next_run_at: Option<DateTime<Utc>>,
    ) -> Result<()> {
        claims::persist_post_tick_job_state(self, job_id, next_run_at).await
    }

    async fn try_acquire_leader(&self, instance_id: &str, ttl_secs: i64) -> Result<bool> {
        coordinator::try_acquire_leader(self, instance_id, ttl_secs).await
    }

    async fn renew_leader_lease(&self, instance_id: &str, ttl_secs: i64) -> Result<()> {
        coordinator::renew_leader_lease(self, instance_id, ttl_secs).await
    }

    async fn get_leader(&self) -> Result<Option<SchedulerLeader>> {
        coordinator::get_leader(self).await
    }

    async fn upsert_partition_assignment(&self, assignment: &PartitionAssignment) -> Result<()> {
        coordinator::upsert_partition_assignment(self, assignment).await
    }

    async fn list_partition_assignments(&self) -> Result<Vec<PartitionAssignment>> {
        coordinator::list_partition_assignments(self).await
    }

    async fn register_worker(&self, worker: &Worker) -> Result<()> {
        coordinator::register_worker(self, worker).await
    }

    async fn heartbeat_worker(&self, worker_id: &str, at: DateTime<Utc>) -> Result<()> {
        coordinator::heartbeat_worker(self, worker_id, at).await
    }
}
