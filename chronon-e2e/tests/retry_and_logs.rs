//! E2E: automatic retry after failure and log capture on failed runs.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;
use std::time::Duration;

use chronon_core::models::{RetryPolicy, RunStatus};
use chronon_testkit::{
    smoke_actor_json, upsert_immediate_run_once_job, wait_for_run_terminal, BootstrapSession,
    MatrixSpec, FAIL_SCRIPT,
};
use tokio::time::sleep;

#[tokio::test]
async fn fail_then_retry_until_success_when_policy_allows() {
    // Use a fail script with max_attempts so we get multiple Failed runs then stop.
    let mut session = BootstrapSession::new(MatrixSpec::ci_mem_embedded());
    session.install().await.expect("bootstrap");
    session.spawn_embedded().await.expect("spawn");

    let store = session.store_dyn().expect("store");
    let job_name = "e2e-retry-exhaust";
    let mut job = upsert_immediate_run_once_job(store.as_ref(), job_name, FAIL_SCRIPT)
        .await
        .expect("upsert");
    job.actor_json = smoke_actor_json();
    job.set_retry_policy(&RetryPolicy {
        max_attempts: 2,
        base_delay_ms: 20,
        backoff_multiplier: 1.0,
        max_delay_ms: 100,
    });
    // run_once jobs normally only fire once; after terminal they won't re-schedule from
    // cron — retries create new Queued runs directly via finalize_failed_run.
    store.upsert_job(&job).await.expect("upsert policy");

    // Wait until we have 3 terminal Failed runs (attempt 1 + 2 retries) or timeout.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    let mut failed_count = 0usize;
    while tokio::time::Instant::now() < deadline {
        let runs = store.list_runs_for_job(&job.job_id, 20).await.unwrap();
        failed_count = runs
            .iter()
            .filter(|r| r.status == RunStatus::Failed)
            .count();
        if failed_count >= 3 {
            break;
        }
        sleep(Duration::from_millis(50)).await;
    }
    assert!(
        failed_count >= 3,
        "expected 3 failed attempts (1 + 2 retries), got {failed_count}"
    );
    let runs = store.list_runs_for_job(&job.job_id, 20).await.unwrap();
    assert!(
        !runs.iter().any(|r| r.status == RunStatus::Queued),
        "no runaway queued runs after exhaustion"
    );
    // At least one failed run must have captured stderr (log flush).
    assert!(
        runs.iter().any(|r| {
            r.status == RunStatus::Failed
                && r.stderr_text
                    .as_deref()
                    .is_some_and(|s| s.contains("probe") || s.contains("fail"))
        }),
        "failed run missing stderr capture: {:?}",
        runs.iter().map(|r| &r.stderr_text).collect::<Vec<_>>()
    );

    session.shutdown_embedded().await.expect("shutdown");
}

#[tokio::test]
async fn failed_run_persists_stderr_logs() {
    let mut session = BootstrapSession::new(MatrixSpec::ci_mem_embedded());
    session.install().await.expect("bootstrap");
    session.spawn_embedded().await.expect("spawn");

    let store = session.store_dyn().expect("store");
    let job_name = "e2e-fail-logs";
    let mut job = upsert_immediate_run_once_job(store.as_ref(), job_name, FAIL_SCRIPT)
        .await
        .expect("upsert");
    job.actor_json = smoke_actor_json();
    // No retries — single terminal failure.
    job.set_retry_policy(&RetryPolicy::default());
    store.upsert_job(&job).await.expect("upsert");

    wait_for_run_terminal(
        Arc::clone(&store),
        job_name,
        RunStatus::Failed,
        Duration::from_secs(5),
    )
    .await
    .expect("failed terminal");

    let runs = store.list_runs_for_job(&job.job_id, 5).await.unwrap();
    let failed = runs
        .iter()
        .find(|r| r.status == RunStatus::Failed)
        .expect("failed run");
    assert!(
        failed.stderr_text.as_deref().is_some_and(|s| !s.is_empty()),
        "stderr_text empty on failed run"
    );

    session.shutdown_embedded().await.expect("shutdown");
}
