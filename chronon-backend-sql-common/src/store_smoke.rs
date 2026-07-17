//! SQLite smoke tests exercising SQL store modules end-to-end.

use chrono::Utc;
use chronon_core::models::{Job, Run, RunStatus, ScheduleKind};
use chronon_core::store::SchedulerStore;

use crate::SqlSchedulerStore;

async fn memory_store() -> SqlSchedulerStore {
    SqlSchedulerStore::connect_sqlite("sqlite://:memory:")
        .await
        .expect("sqlite connect")
}

#[tokio::test]
async fn jobs_upsert_get_roundtrip() {
    let store = memory_store().await;
    let job = Job::new("smoke-job", "script_a");
    let job_id = job.job_id.clone();
    store.upsert_job(&job).await.expect("upsert");
    let fetched = store.get_job(&job_id).await.expect("get").expect("job");
    assert_eq!(fetched.job_name, "smoke-job");
}

#[tokio::test]
async fn runs_create_and_claim() {
    let store = memory_store().await;
    let job = Job::new("run-job", "script_a");
    store.upsert_job(&job).await.expect("upsert job");
    let run = Run {
        run_id: "run-smoke".into(),
        job_id: Some(job.job_id.clone()),
        script_name: "script_a".into(),
        parent_run_id: None,
        root_run_id: None,
        child_index: None,
        scheduled_for: Utc::now(),
        started_at: None,
        finished_at: None,
        duration_ms: None,
        status: RunStatus::Queued,
        attempt: 1,
        instance_id: None,
        placement_json: None,
        pool_id: Some("general".into()),
        actor_json: serde_json::json!({}),
        params_json: serde_json::json!({}),
        stdout_text: None,
        stderr_text: None,
        error_json: None,
        stats_json: None,
        claimed_by: None,
        claim_lease_until: None,
    };
    store.create_run(&run).await.expect("create run");
    let claimed = store
        .claim_next_queued("general", "worker-1", Utc::now(), 30)
        .await
        .expect("claim");
    assert!(claimed.is_some());
}

#[tokio::test]
async fn run_once_claim_smoke() {
    let store = memory_store().await;
    let mut job = Job::new("once-job", "script_a");
    job.schedule_kind = ScheduleKind::RunOnce;
    job.run_once_at = Some(Utc::now());
    store.upsert_job(&job).await.expect("upsert");
    let claimed = store
        .try_claim_run_once(&job.job_id, "worker-1", Utc::now(), 60)
        .await
        .expect("claim run-once");
    assert!(claimed);
}

#[tokio::test]
async fn leader_acquire_smoke() {
    let store = memory_store().await;
    let acquired = store
        .try_acquire_leader("inst-1", 30)
        .await
        .expect("acquire");
    assert!(acquired);
    let leader = store.get_leader().await.expect("leader").expect("row");
    assert_eq!(leader.leader_instance_id, "inst-1");
}
