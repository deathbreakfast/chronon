use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::json;

use crate::context::{ContextFactory, NoOpContextFactory, ScriptContext};
use crate::error::Result;
use crate::models::{
    ChildOutcome, ChildSet, Job, JobRevision, PartitionAssignment, Run, RunStatus, SchedulerLeader,
    Script, ShardSet, Worker,
};
use crate::router::{StoreRouter, DEFAULT_STORE_NAME};
use crate::store::SchedulerStore;
use crate::ChrononError;

struct EmptyStore;

#[async_trait]
impl SchedulerStore for EmptyStore {
    async fn upsert_job(&self, _: &Job) -> Result<()> {
        Ok(())
    }
    async fn get_job(&self, _: &str) -> Result<Option<Job>> {
        Ok(None)
    }
    async fn get_job_by_name(&self, _: &str) -> Result<Option<Job>> {
        Ok(None)
    }
    async fn list_jobs(&self) -> Result<Vec<Job>> {
        Ok(vec![])
    }
    async fn list_due_jobs(&self, _: DateTime<Utc>) -> Result<Vec<Job>> {
        Ok(vec![])
    }
    async fn pause_job(&self, _: &str) -> Result<()> {
        Ok(())
    }
    async fn resume_job(&self, _: &str) -> Result<()> {
        Ok(())
    }
    async fn create_run(&self, _: &Run) -> Result<()> {
        Ok(())
    }
    async fn update_run(&self, _: &Run) -> Result<()> {
        Ok(())
    }
    async fn get_run(&self, _: &str) -> Result<Option<Run>> {
        Ok(None)
    }
    async fn list_runs_for_job(&self, _: &str, _: usize) -> Result<Vec<Run>> {
        Ok(vec![])
    }
    async fn list_runs_filtered(
        &self,
        _: Option<&str>,
        _: Option<RunStatus>,
        _: usize,
        _: usize,
    ) -> Result<Vec<Run>> {
        Ok(vec![])
    }
    async fn claim_next_queued(
        &self,
        _: &str,
        _: &str,
        _: DateTime<Utc>,
        _: i64,
    ) -> Result<Option<Run>> {
        Ok(None)
    }
    async fn claim_run_by_id(
        &self,
        _: &str,
        _: &str,
        _: &str,
        _: DateTime<Utc>,
        _: i64,
    ) -> Result<Option<Run>> {
        Ok(None)
    }
    async fn renew_run_lease(&self, _: &str, _: &str, _: DateTime<Utc>, _: i64) -> Result<bool> {
        Ok(false)
    }
    async fn reclaim_expired_run_leases(&self, _: DateTime<Utc>) -> Result<Vec<String>> {
        Ok(vec![])
    }
    async fn append_revision(&self, _: &JobRevision) -> Result<()> {
        Ok(())
    }
    async fn list_revisions(&self, _: &str) -> Result<Vec<JobRevision>> {
        Ok(vec![])
    }
    async fn upsert_script(&self, _: &Script) -> Result<()> {
        Ok(())
    }
    async fn get_script(&self, _: &str) -> Result<Option<Script>> {
        Ok(None)
    }
    async fn try_claim_run_once(&self, _: &str, _: &str, _: DateTime<Utc>, _: i64) -> Result<bool> {
        Ok(false)
    }
    async fn mark_run_once_completed(&self, _: &str, _: DateTime<Utc>) -> Result<()> {
        Ok(())
    }
    async fn release_run_once_claim(&self, _: &str, _: &str, _: DateTime<Utc>) -> Result<()> {
        Ok(())
    }
    async fn find_due_job_ids_in_partitions(
        &self,
        _: &[u32],
        _: DateTime<Utc>,
        _: u32,
    ) -> Result<Vec<String>> {
        Ok(vec![])
    }
    async fn min_next_run_at_in_partitions(&self, _: &[u32]) -> Result<Option<DateTime<Utc>>> {
        Ok(None)
    }
    async fn claim_job_for_tick(&self, _: &str, _: &str, _: DateTime<Utc>, _: i64) -> Result<bool> {
        Ok(false)
    }
    async fn release_job_tick_claim(&self, _: &str) -> Result<()> {
        Ok(())
    }
    async fn persist_post_tick_job_state(&self, _: &str, _: Option<DateTime<Utc>>) -> Result<()> {
        Ok(())
    }
    async fn try_acquire_leader(&self, _: &str, _: i64) -> Result<bool> {
        Ok(false)
    }
    async fn renew_leader_lease(&self, _: &str, _: i64) -> Result<()> {
        Ok(())
    }
    async fn get_leader(&self) -> Result<Option<SchedulerLeader>> {
        Ok(None)
    }
    async fn upsert_partition_assignment(&self, _: &PartitionAssignment) -> Result<()> {
        Ok(())
    }
    async fn list_partition_assignments(&self) -> Result<Vec<PartitionAssignment>> {
        Ok(vec![])
    }
    async fn register_worker(&self, _: &Worker) -> Result<()> {
        Ok(())
    }
    async fn heartbeat_worker(&self, _: &str, _: DateTime<Utc>) -> Result<()> {
        Ok(())
    }
    async fn set_run_outputs(&self, run_id: &str, _: &serde_json::Value) -> Result<()> {
        Err(ChrononError::RunNotFound(run_id.to_string()))
    }
    async fn count_active_runs_for_job(&self, _: &str) -> Result<i64> {
        Ok(0)
    }
    async fn create_child_set_with_runs(&self, _: &ChildSet, _: &[Run]) -> Result<()> {
        Ok(())
    }
    async fn get_child_set(&self, _: &str) -> Result<Option<ChildSet>> {
        Ok(None)
    }
    async fn list_child_sets_for_run(&self, _: &str) -> Result<Vec<ChildSet>> {
        Ok(vec![])
    }
    async fn record_child_result(&self, set_id: &str, _: ChildOutcome) -> Result<ChildSet> {
        Err(ChrononError::storage(format!(
            "child set not found: {set_id}"
        )))
    }
    async fn settle_child_set(&self, _: &str, _: DateTime<Utc>) -> Result<()> {
        Ok(())
    }
    async fn create_shard_set(&self, _: &ShardSet) -> Result<()> {
        Ok(())
    }
    async fn get_shard_set(&self, _: &str) -> Result<Option<ShardSet>> {
        Ok(None)
    }
    async fn update_shard_set(&self, _: &ShardSet, _: &serde_json::Value) -> Result<bool> {
        Ok(false)
    }
    async fn set_shard_checkpoint(
        &self,
        set_id: &str,
        _: u32,
        _: &serde_json::Value,
    ) -> Result<()> {
        Err(ChrononError::storage(format!(
            "shard set not found: {set_id}"
        )))
    }
}

#[tokio::test]
async fn empty_store_answers_set_methods() {
    let store = EmptyStore;
    assert_eq!(store.count_active_runs_for_job("j").await.unwrap(), 0);
    assert!(store.get_child_set("s").await.unwrap().is_none());
    assert!(store.get_shard_set("s").await.unwrap().is_none());
    assert!(store
        .record_child_result("s", ChildOutcome::Succeeded)
        .await
        .is_err());
}

#[test]
fn job_new_has_defaults() {
    let job = Job::new("daily", "cleanup");
    assert!(job.enabled);
    assert_eq!(job.current_revision, 1);
}

#[test]
fn run_status_helpers() {
    assert!(RunStatus::Success.is_terminal());
    assert!(!RunStatus::Queued.is_terminal());
    assert!(RunStatus::Queued.is_active());
}

#[test]
fn run_lifecycle() {
    let mut run = Run::for_job("job-1", "script", Utc::now());
    run.start();
    assert_eq!(run.status, RunStatus::Running);
    run.complete();
    assert!(run.duration_ms.is_some());
}

#[test]
fn store_router_register_and_get() {
    let store = Arc::new(EmptyStore);
    let mut router = StoreRouter::new();
    router.register(DEFAULT_STORE_NAME, store);
    assert!(router.get(DEFAULT_STORE_NAME).is_some());
}

#[test]
fn store_router_missing_default_is_none() {
    let router = StoreRouter::new();
    assert!(router.get(DEFAULT_STORE_NAME).is_none());
}

#[test]
fn noop_context_factory() {
    let factory = NoOpContextFactory;
    let ctx = factory.build(&json!({"actor": "test"})).expect("build");
    assert_eq!(ctx.label(), "noop");
    let _: Box<dyn ScriptContext> = ctx;
}

#[test]
fn identity_error_maps_to_chronon_error() {
    let err: ChrononError = crate::IdentityError("bad actor".into()).into();
    assert!(matches!(err, ChrononError::Identity(_)));
}

#[test]
fn job_json_roundtrip() {
    let job = Job::new("nightly", "cleanup");
    let json = serde_json::to_string(&job).expect("serialize");
    let back: Job = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back.job_name, "nightly");
}
