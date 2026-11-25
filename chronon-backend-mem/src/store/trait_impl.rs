//! [`SchedulerStore`] trait surface for [`InMemorySchedulerStore`](super::InMemorySchedulerStore).

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use chronon_core::models::{
    ChildOutcome, ChildSet, Job, JobRevision, PartitionAssignment, Run, RunKey, RunStatus,
    SchedulerLeader, Script, ShardSet, Worker,
};
use chronon_core::store::SchedulerStore;
use chronon_core::Result;

use serde_json::Value;

use super::{claims, coordinator, jobs, runs, sets, InMemorySchedulerStore};

#[async_trait]
impl SchedulerStore for InMemorySchedulerStore {
    async fn upsert_job(&self, job: &Job) -> Result<()> {
        jobs::upsert_job(self, job)
    }

    async fn get_job(&self, job_id: &str) -> Result<Option<Job>> {
        jobs::get_job(self, job_id)
    }

    async fn get_job_by_name(&self, job_name: &str) -> Result<Option<Job>> {
        jobs::get_job_by_name(self, job_name)
    }

    async fn list_jobs(&self) -> Result<Vec<Job>> {
        jobs::list_jobs(self)
    }

    async fn list_due_jobs(&self, before: DateTime<Utc>) -> Result<Vec<Job>> {
        jobs::list_due_jobs(self, before)
    }

    async fn pause_job(&self, job_id: &str) -> Result<()> {
        jobs::pause_job(self, job_id)
    }

    async fn delete_job(&self, job_id: &str) -> Result<bool> {
        jobs::delete_job(self, job_id)
    }

    async fn resume_job(&self, job_id: &str) -> Result<()> {
        jobs::resume_job(self, job_id)
    }

    async fn create_run(&self, run: &Run) -> Result<()> {
        runs::create_run(self, run)
    }

    async fn update_run(&self, run: &Run) -> Result<()> {
        runs::update_run(self, run)
    }

    async fn get_run(&self, run_id: &str) -> Result<Option<Run>> {
        runs::get_run(self, run_id)
    }

    async fn list_runs_for_job(&self, job_id: &str, limit: usize) -> Result<Vec<Run>> {
        runs::list_runs_for_job(self, job_id, limit)
    }

    async fn list_runs_filtered(
        &self,
        job_id: Option<&str>,
        status: Option<RunStatus>,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<Run>> {
        runs::list_runs_filtered(self, job_id, status, offset, limit)
    }

    async fn list_runs_after(
        &self,
        job_id: Option<&str>,
        status: Option<RunStatus>,
        after: Option<&RunKey>,
        limit: usize,
    ) -> Result<Vec<Run>> {
        runs::list_runs_after(self, job_id, status, after, limit)
    }

    async fn claim_next_queued(
        &self,
        pool_id: &str,
        worker_id: &str,
        now: DateTime<Utc>,
        lease_ttl_secs: i64,
    ) -> Result<Option<Run>> {
        runs::claim_next_queued(self, pool_id, worker_id, now, lease_ttl_secs)
    }

    async fn claim_run_by_id(
        &self,
        run_id: &str,
        pool_id: &str,
        worker_id: &str,
        now: DateTime<Utc>,
        lease_ttl_secs: i64,
    ) -> Result<Option<Run>> {
        runs::claim_run_by_id(self, run_id, pool_id, worker_id, now, lease_ttl_secs)
    }

    async fn renew_run_lease(
        &self,
        run_id: &str,
        worker_id: &str,
        now: DateTime<Utc>,
        lease_ttl_secs: i64,
    ) -> Result<bool> {
        runs::renew_run_lease(self, run_id, worker_id, now, lease_ttl_secs)
    }

    async fn reclaim_expired_run_leases(&self, now: DateTime<Utc>) -> Result<Vec<String>> {
        runs::reclaim_expired_run_leases(self, now)
    }

    async fn set_run_outputs(&self, run_id: &str, outputs: &Value) -> Result<()> {
        sets::set_run_outputs(self, run_id, outputs)
    }

    async fn count_active_runs_for_job(&self, job_id: &str) -> Result<i64> {
        sets::count_active_runs_for_job(self, job_id)
    }

    async fn create_child_set_with_runs(&self, set: &ChildSet, runs: &[Run]) -> Result<()> {
        sets::create_child_set_with_runs(self, set, runs)
    }

    async fn get_child_set(&self, set_id: &str) -> Result<Option<ChildSet>> {
        sets::get_child_set(self, set_id)
    }

    async fn list_child_sets_for_run(&self, parent_run_id: &str) -> Result<Vec<ChildSet>> {
        sets::list_child_sets_for_run(self, parent_run_id)
    }

    async fn record_child_result(&self, set_id: &str, outcome: ChildOutcome) -> Result<ChildSet> {
        sets::record_child_result(self, set_id, outcome)
    }

    async fn settle_child_set(&self, set_id: &str, at: DateTime<Utc>) -> Result<()> {
        sets::settle_child_set(self, set_id, at)
    }

    async fn create_shard_set(&self, set: &ShardSet) -> Result<()> {
        sets::create_shard_set(self, set)
    }

    async fn get_shard_set(&self, set_id: &str) -> Result<Option<ShardSet>> {
        sets::get_shard_set(self, set_id)
    }

    async fn update_shard_set(&self, set: &ShardSet, expected_next_start: &Value) -> Result<bool> {
        sets::update_shard_set(self, set, expected_next_start)
    }

    async fn set_shard_checkpoint(
        &self,
        set_id: &str,
        shard_index: u32,
        value: &Value,
    ) -> Result<()> {
        sets::set_shard_checkpoint(self, set_id, shard_index, value)
    }

    async fn append_revision(&self, revision: &JobRevision) -> Result<()> {
        coordinator::append_revision(self, revision)
    }

    async fn list_revisions(&self, job_id: &str) -> Result<Vec<JobRevision>> {
        coordinator::list_revisions(self, job_id)
    }

    async fn upsert_script(&self, script: &Script) -> Result<()> {
        coordinator::upsert_script(self, script)
    }

    async fn get_script(&self, script_name: &str) -> Result<Option<Script>> {
        coordinator::get_script(self, script_name)
    }

    async fn try_claim_run_once(
        &self,
        job_id: &str,
        claimed_by: &str,
        now: DateTime<Utc>,
        claim_ttl_secs: i64,
    ) -> Result<bool> {
        claims::try_claim_run_once(self, job_id, claimed_by, now, claim_ttl_secs)
    }

    async fn mark_run_once_completed(
        &self,
        job_id: &str,
        completed_at: DateTime<Utc>,
    ) -> Result<()> {
        claims::mark_run_once_completed(self, job_id, completed_at)
    }

    async fn release_run_once_claim(
        &self,
        job_id: &str,
        claimed_by: &str,
        now: DateTime<Utc>,
    ) -> Result<()> {
        claims::release_run_once_claim(self, job_id, claimed_by, now)
    }

    async fn find_due_job_ids_in_partitions(
        &self,
        owned_partitions: &[u32],
        due_until: DateTime<Utc>,
        limit: u32,
    ) -> Result<Vec<String>> {
        claims::find_due_job_ids_in_partitions(self, owned_partitions, due_until, limit)
    }

    async fn min_next_run_at_in_partitions(
        &self,
        owned_partitions: &[u32],
    ) -> Result<Option<DateTime<Utc>>> {
        claims::min_next_run_at_in_partitions(self, owned_partitions)
    }

    async fn claim_job_for_tick(
        &self,
        job_id: &str,
        claim_id: &str,
        now: DateTime<Utc>,
        lease_ttl_secs: i64,
    ) -> Result<bool> {
        claims::claim_job_for_tick(self, job_id, claim_id, now, lease_ttl_secs)
    }

    async fn release_job_tick_claim(&self, job_id: &str) -> Result<()> {
        claims::release_job_tick_claim(self, job_id)
    }

    async fn persist_post_tick_job_state(
        &self,
        job_id: &str,
        next_run_at: Option<DateTime<Utc>>,
    ) -> Result<()> {
        claims::persist_post_tick_job_state(self, job_id, next_run_at)
    }

    async fn try_acquire_leader(&self, instance_id: &str, ttl_secs: i64) -> Result<bool> {
        coordinator::try_acquire_leader(self, instance_id, ttl_secs)
    }

    async fn renew_leader_lease(&self, instance_id: &str, ttl_secs: i64) -> Result<()> {
        coordinator::renew_leader_lease(self, instance_id, ttl_secs)
    }

    async fn get_leader(&self) -> Result<Option<SchedulerLeader>> {
        coordinator::get_leader(self)
    }

    async fn upsert_partition_assignment(&self, assignment: &PartitionAssignment) -> Result<()> {
        coordinator::upsert_partition_assignment(self, assignment)
    }

    async fn list_partition_assignments(&self) -> Result<Vec<PartitionAssignment>> {
        coordinator::list_partition_assignments(self)
    }

    async fn register_worker(&self, worker: &Worker) -> Result<()> {
        coordinator::register_worker(self, worker)
    }

    async fn heartbeat_worker(&self, worker_id: &str, at: DateTime<Utc>) -> Result<()> {
        coordinator::heartbeat_worker(self, worker_id, at)
    }
}
