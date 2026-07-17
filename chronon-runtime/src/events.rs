//! Persist executor lifecycle events to the store.

use std::sync::Arc;

use chrono::Utc;
use chronon_core::models::RunStatus;
use chronon_core::store::SchedulerStore;
use chronon_executor::ExecutorEvent;

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
        ExecutorEvent::RunCompleted { run_id, duration_ms } => {
            if let Ok(Some(mut run)) = store.get_run(&run_id).await {
                run.complete();
                run.duration_ms = Some(duration_ms);
                let _ = store.update_run(&run).await;
            }
        }
        ExecutorEvent::RunFailed { run_id, error } => {
            if let Ok(Some(run)) = store.get_run(&run_id).await {
                let job = match run.job_id.as_deref() {
                    Some(id) => store.get_job(id).await.ok().flatten(),
                    None => None,
                };
                if let Some(job) = job {
                    finalize_failed_run(store, run, &job, RunStatus::Failed, error).await;
                } else {
                    let mut run = run;
                    run.fail(error);
                    let _ = store.update_run(&run).await;
                }
            }
        }
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
