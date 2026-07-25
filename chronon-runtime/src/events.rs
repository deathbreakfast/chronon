//! Persist executor lifecycle events to the store.

use std::sync::Arc;

use chrono::Utc;
use chronon_core::models::RunStatus;
use chronon_core::store::SchedulerStore;
use chronon_executor::ExecutorEvent;
use chronon_telemetry::CapturedLogs;

use crate::retry::finalize_failed_run;

/// Apply one executor lifecycle event to the run row in `store`.
pub async fn handle_executor_event(store: &Arc<dyn SchedulerStore>, event: ExecutorEvent) {
    match event {
        ExecutorEvent::RunStarted { run_id } => {
            if let Ok(Some(mut run)) = store.get_run(&run_id).await {
                run.started_at = Some(Utc::now());
                run.status = RunStatus::Running;
                let _ = store.update_run(&run).await;
            }
        }
        ExecutorEvent::RunCompleted {
            run_id,
            duration_ms,
            logs,
        } => {
            if let Ok(Some(mut run)) = store.get_run(&run_id).await {
                run.complete();
                run.duration_ms = Some(duration_ms);
                apply_logs(&mut run, logs);
                let _ = store.update_run(&run).await;
            }
        }
        ExecutorEvent::RunFailed {
            run_id,
            error,
            logs,
        } => {
            if let Ok(Some(run)) = store.get_run(&run_id).await {
                let job = match run.job_id.as_deref() {
                    Some(id) => store.get_job(id).await.ok().flatten(),
                    None => None,
                };
                if let Some(job) = job {
                    finalize_failed_run(
                        store,
                        run,
                        &job,
                        RunStatus::Failed,
                        error,
                        Some(logs),
                    )
                    .await;
                } else {
                    let mut run = run;
                    run.fail(error.clone());
                    let mut captured = logs;
                    captured.ensure_stderr_message(&error);
                    apply_logs(&mut run, captured);
                    let _ = store.update_run(&run).await;
                }
            }
        }
    }
}

fn apply_logs(run: &mut chronon_core::models::Run, logs: CapturedLogs) {
    if logs.stdout_text.is_some() {
        run.stdout_text = logs.stdout_text;
    }
    if logs.stderr_text.is_some() {
        run.stderr_text = logs.stderr_text;
    }
}

/// Background task: persist executor events until the channel closes.
pub fn spawn_event_handler(
    store: Arc<dyn SchedulerStore>,
    mut event_rx: tokio::sync::mpsc::UnboundedReceiver<ExecutorEvent>,
) {
    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            handle_executor_event(&store, event).await;
        }
    });
}
