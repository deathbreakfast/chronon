//! Finalize failed/timed-out runs and enqueue delayed retries.

use std::sync::Arc;

use chrono::{Duration, Utc};
use chronon_core::models::{Job, Run, RunStatus};
use chronon_core::store::SchedulerStore;

/// Persist a terminal failure/timeout and enqueue a retry when the job policy allows.
pub async fn finalize_failed_run(
    store: &Arc<dyn SchedulerStore>,
    mut run: Run,
    job: &Job,
    status: RunStatus,
    error: impl Into<String>,
) {
    let message = error.into();
    match status {
        RunStatus::Timeout => run.timeout(message),
        _ => run.fail(message),
    }
    let _ = store.update_run(&run).await;

    let policy = job.retry_policy();
    if !policy.should_retry(run.attempt) {
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
    let _ = store.create_run(&next).await;
}
