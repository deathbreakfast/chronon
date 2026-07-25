//! Finalize failed/timed-out runs and enqueue delayed retries.
//!
//! # Concern → API
//!
//! | Concern | API |
//! |---------|-----|
//! | Persist failure + schedule retry | [`finalize_failed_run`] |
//! | Policy decode / backoff | [`chronon_core::RetryPolicy`] on the job |
//!
//! # Examples
//!
//! ```ignore
//! use chronon_core::models::{RetryPolicy, RunStatus};
//! use chronon_runtime::retry::finalize_failed_run;
//!
//! // After a worker script returns Err or times out:
//! finalize_failed_run(
//!     &store,
//!     run,
//!     &job,
//!     RunStatus::Failed,
//!     "script exploded",
//!     None,
//! ).await;
//! // When `job.retry_policy().should_retry(run.attempt)`, a new Queued run
//! // is created with `attempt + 1` and `scheduled_for = now + delay`.
//! ```

use std::sync::Arc;

use chrono::{Duration, Utc};
use chronon_core::models::{Job, Run, RunStatus};
use chronon_core::store::SchedulerStore;
use chronon_telemetry::CapturedLogs;
use tracing::{info, warn};

/// Persist a terminal failure/timeout and enqueue a retry when the job policy allows.
///
/// Applies optional [`CapturedLogs`] to the failed run before update. Does **not** retry
/// `Canceled` or `Success` (callers must only pass [`RunStatus::Failed`] or
/// [`RunStatus::Timeout`]).
pub async fn finalize_failed_run(
    store: &Arc<dyn SchedulerStore>,
    mut run: Run,
    job: &Job,
    status: RunStatus,
    error: impl Into<String>,
    logs: Option<CapturedLogs>,
) {
    let message = error.into();
    match status {
        RunStatus::Timeout => run.timeout(message.clone()),
        _ => run.fail(message.clone()),
    }
    if let Some(mut captured) = logs {
        captured.ensure_stderr_message(&message);
        run.stdout_text = captured.stdout_text;
        run.stderr_text = captured.stderr_text;
    } else {
        run.stderr_text = Some(message.clone());
    }

    if let Err(e) = store.update_run(&run).await {
        warn!(
            run_id = %run.run_id,
            job_id = ?run.job_id,
            error = %e,
            "failed to persist terminal run before retry decision"
        );
        return;
    }

    let policy = job.retry_policy();
    if !policy.should_retry(run.attempt) {
        info!(
            run_id = %run.run_id,
            attempt = run.attempt,
            max_attempts = policy.max_attempts,
            "retry policy exhausted or disabled; no further attempt"
        );
        return;
    }

    let delay_ms = policy.delay_ms_after(run.attempt) as i64;
    let scheduled_for = Utc::now() + Duration::milliseconds(delay_ms.max(0));
    let mut next = Run::for_job(
        run.job_id.clone().unwrap_or_default(),
        &run.script_name,
        scheduled_for,
    );
    next.attempt = run.attempt + 1;
    next.actor_json = run.actor_json.clone();
    next.params_json = run.params_json.clone();
    next.pool_id = run.pool_id.clone();
    next.placement_json = run.placement_json.clone();
    next.parent_run_id = run.parent_run_id.clone();
    next.root_run_id = run.root_run_id.clone();

    match store.create_run(&next).await {
        Ok(()) => {
            info!(
                prior_run_id = %run.run_id,
                next_run_id = %next.run_id,
                next_attempt = next.attempt,
                delay_ms,
                "enqueued retry run"
            );
        }
        Err(e) => {
            warn!(
                prior_run_id = %run.run_id,
                next_attempt = next.attempt,
                error = %e,
                "failed to create retry run"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use chronon_backend_mem::InMemorySchedulerStore;
    use chronon_core::models::{Job, RetryPolicy, RunStatus};
    use chronon_core::store::SchedulerStore;

    fn job_with_retries(max_attempts: u32, base_delay_ms: u64) -> Job {
        let mut job = Job::new("retry-job", "script");
        job.set_retry_policy(&RetryPolicy {
            max_attempts,
            base_delay_ms,
            backoff_multiplier: 2.0,
            max_delay_ms: 10_000,
        });
        job
    }

    #[tokio::test]
    async fn failed_run_schedules_retry_with_incremented_attempt() {
        let store: Arc<dyn SchedulerStore> = Arc::new(InMemorySchedulerStore::new());
        let job = job_with_retries(3, 100);
        store.upsert_job(&job).await.unwrap();

        let run = Run::for_job(&job.job_id, "script", Utc::now());
        assert_eq!(run.attempt, 1);
        store.create_run(&run).await.unwrap();

        finalize_failed_run(
            &store,
            run.clone(),
            &job,
            RunStatus::Failed,
            "boom",
            Some(CapturedLogs {
                stdout_text: Some("out".into()),
                stderr_text: None,
            }),
        )
        .await;

        let failed = store.get_run(&run.run_id).await.unwrap().unwrap();
        assert_eq!(failed.status, RunStatus::Failed);
        assert_eq!(failed.stdout_text.as_deref(), Some("out"));
        assert_eq!(failed.stderr_text.as_deref(), Some("boom"));

        let runs = store.list_runs_for_job(&job.job_id, 10).await.unwrap();
        assert_eq!(runs.len(), 2);
        let next = runs
            .iter()
            .find(|r| r.run_id != run.run_id)
            .expect("retry run");
        assert_eq!(next.attempt, 2);
        assert_eq!(next.status, RunStatus::Queued);
        let delta = (next.scheduled_for - Utc::now()).num_milliseconds();
        assert!((50..=250).contains(&delta), "delay ~100ms, got {delta}");
    }

    #[tokio::test]
    async fn timeout_also_retries() {
        let store: Arc<dyn SchedulerStore> = Arc::new(InMemorySchedulerStore::new());
        let job = job_with_retries(1, 0);
        store.upsert_job(&job).await.unwrap();
        let run = Run::for_job(&job.job_id, "script", Utc::now());
        store.create_run(&run).await.unwrap();

        finalize_failed_run(&store, run, &job, RunStatus::Timeout, "timeout", None).await;

        let runs = store.list_runs_for_job(&job.job_id, 10).await.unwrap();
        assert_eq!(runs.len(), 2);
        assert!(runs.iter().any(|r| r.status == RunStatus::Timeout));
        assert!(runs
            .iter()
            .any(|r| r.status == RunStatus::Queued && r.attempt == 2));
    }

    #[tokio::test]
    async fn exhausted_attempts_do_not_enqueue() {
        let store: Arc<dyn SchedulerStore> = Arc::new(InMemorySchedulerStore::new());
        let job = job_with_retries(1, 0);
        store.upsert_job(&job).await.unwrap();
        let mut run = Run::for_job(&job.job_id, "script", Utc::now());
        run.attempt = 2; // first retry already used (max_attempts=1)
        store.create_run(&run).await.unwrap();

        finalize_failed_run(&store, run, &job, RunStatus::Failed, "done", None).await;

        let runs = store.list_runs_for_job(&job.job_id, 10).await.unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].status, RunStatus::Failed);
    }

    #[tokio::test]
    async fn default_policy_does_not_retry() {
        let store: Arc<dyn SchedulerStore> = Arc::new(InMemorySchedulerStore::new());
        let job = Job::new("no-retry", "script");
        store.upsert_job(&job).await.unwrap();
        let run = Run::for_job(&job.job_id, "script", Utc::now());
        store.create_run(&run).await.unwrap();

        finalize_failed_run(&store, run, &job, RunStatus::Failed, "x", None).await;

        assert_eq!(
            store
                .list_runs_for_job(&job.job_id, 10)
                .await
                .unwrap()
                .len(),
            1
        );
    }
}
