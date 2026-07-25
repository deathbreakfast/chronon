//! Persist executor lifecycle events to the store.

use std::sync::Arc;

use chrono::Utc;
use chronon_core::models::RunStatus;
use chronon_core::store::SchedulerStore;
use chronon_executor::ExecutorEvent;
use chronon_telemetry::CapturedLogs;
use tracing::debug;

use crate::retry::finalize_failed_run;

/// Apply one executor lifecycle event to the run row in `store`.
///
/// Illegal status transitions are ignored (forged or late events must not rewind or
/// complete a run that was never started).
pub async fn handle_executor_event(store: &Arc<dyn SchedulerStore>, event: ExecutorEvent) {
    match event {
        ExecutorEvent::RunStarted { run_id } => {
            if let Ok(Some(mut run)) = store.get_run(&run_id).await {
                if !matches!(run.status, RunStatus::Queued | RunStatus::Claimed) {
                    debug!(
                        run_id = %run_id,
                        status = %run.status,
                        "ignoring RunStarted for non-startable run"
                    );
                    return;
                }
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
                if run.status != RunStatus::Running {
                    debug!(
                        run_id = %run_id,
                        status = %run.status,
                        "ignoring RunCompleted for non-running run"
                    );
                    return;
                }
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
                if run.status != RunStatus::Running {
                    debug!(
                        run_id = %run_id,
                        status = %run.status,
                        "ignoring RunFailed for non-running run"
                    );
                    return;
                }
                let job = match run.job_id.as_deref() {
                    Some(id) => store.get_job(id).await.ok().flatten(),
                    None => None,
                };
                if let Some(job) = job {
                    finalize_failed_run(store, run, &job, RunStatus::Failed, error, Some(logs))
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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use std::sync::Arc;

    use chronon_backend_mem::InMemorySchedulerStore;
    use chronon_core::models::{Job, Run, RunStatus, ScheduleKind};
    use chronon_core::store::SchedulerStore;
    use chronon_executor::ExecutorEvent;
    use chronon_telemetry::CapturedLogs;

    use super::handle_executor_event;

    async fn seed_queued_run(store: &Arc<dyn SchedulerStore>) -> (Job, Run) {
        let mut job = Job::new("evt-job", "noop");
        job.schedule_kind = ScheduleKind::Manual;
        store.upsert_job(&job).await.unwrap();
        let mut run = Run::for_job(&job.job_id, &job.script_name, chrono::Utc::now());
        run.actor_json = serde_json::json!({"user": "alice"});
        store.create_run(&run).await.unwrap();
        (job, run)
    }

    #[tokio::test]
    async fn run_started_moves_queued_to_running() {
        let store: Arc<dyn SchedulerStore> = Arc::new(InMemorySchedulerStore::new());
        let (_job, run) = seed_queued_run(&store).await;
        handle_executor_event(
            &store,
            ExecutorEvent::RunStarted {
                run_id: run.run_id.clone(),
            },
        )
        .await;
        let updated = store.get_run(&run.run_id).await.unwrap().unwrap();
        assert_eq!(updated.status, RunStatus::Running);
    }

    #[tokio::test]
    async fn run_completed_on_queued_is_ignored() {
        let store: Arc<dyn SchedulerStore> = Arc::new(InMemorySchedulerStore::new());
        let (_job, run) = seed_queued_run(&store).await;
        handle_executor_event(
            &store,
            ExecutorEvent::RunCompleted {
                run_id: run.run_id.clone(),
                duration_ms: 10,
                logs: CapturedLogs::default(),
            },
        )
        .await;
        let updated = store.get_run(&run.run_id).await.unwrap().unwrap();
        assert_eq!(updated.status, RunStatus::Queued);
        assert!(updated.finished_at.is_none());
    }

    #[tokio::test]
    async fn run_started_from_claimed_moves_to_running() {
        let store: Arc<dyn SchedulerStore> = Arc::new(InMemorySchedulerStore::new());
        let (_job, mut run) = seed_queued_run(&store).await;
        run.status = RunStatus::Claimed;
        store.update_run(&run).await.unwrap();
        handle_executor_event(
            &store,
            ExecutorEvent::RunStarted {
                run_id: run.run_id.clone(),
            },
        )
        .await;
        let updated = store.get_run(&run.run_id).await.unwrap().unwrap();
        assert_eq!(updated.status, RunStatus::Running);
    }

    #[tokio::test]
    async fn run_completed_from_running_succeeds() {
        let store: Arc<dyn SchedulerStore> = Arc::new(InMemorySchedulerStore::new());
        let (_job, mut run) = seed_queued_run(&store).await;
        run.status = RunStatus::Running;
        store.update_run(&run).await.unwrap();
        handle_executor_event(
            &store,
            ExecutorEvent::RunCompleted {
                run_id: run.run_id.clone(),
                duration_ms: 42,
                logs: CapturedLogs::default(),
            },
        )
        .await;
        let updated = store.get_run(&run.run_id).await.unwrap().unwrap();
        assert_eq!(updated.status, RunStatus::Success);
        assert_eq!(updated.duration_ms, Some(42));
    }

    #[tokio::test]
    async fn run_failed_from_running_marks_failed() {
        let store: Arc<dyn SchedulerStore> = Arc::new(InMemorySchedulerStore::new());
        let (_job, mut run) = seed_queued_run(&store).await;
        run.status = RunStatus::Running;
        store.update_run(&run).await.unwrap();
        handle_executor_event(
            &store,
            ExecutorEvent::RunFailed {
                run_id: run.run_id.clone(),
                error: "boom".into(),
                logs: CapturedLogs::default(),
            },
        )
        .await;
        let updated = store.get_run(&run.run_id).await.unwrap().unwrap();
        assert_eq!(updated.status, RunStatus::Failed);
    }

    #[tokio::test]
    async fn run_failed_on_queued_is_ignored() {
        let store: Arc<dyn SchedulerStore> = Arc::new(InMemorySchedulerStore::new());
        let (_job, run) = seed_queued_run(&store).await;
        handle_executor_event(
            &store,
            ExecutorEvent::RunFailed {
                run_id: run.run_id.clone(),
                error: "forged".into(),
                logs: CapturedLogs::default(),
            },
        )
        .await;
        let updated = store.get_run(&run.run_id).await.unwrap().unwrap();
        assert_eq!(updated.status, RunStatus::Queued);
        assert!(updated.finished_at.is_none());
    }

    #[tokio::test]
    async fn run_started_on_terminal_is_ignored() {
        let store: Arc<dyn SchedulerStore> = Arc::new(InMemorySchedulerStore::new());
        let (_job, mut run) = seed_queued_run(&store).await;
        run.status = RunStatus::Success;
        store.update_run(&run).await.unwrap();
        handle_executor_event(
            &store,
            ExecutorEvent::RunStarted {
                run_id: run.run_id.clone(),
            },
        )
        .await;
        let updated = store.get_run(&run.run_id).await.unwrap().unwrap();
        assert_eq!(updated.status, RunStatus::Success);
        assert!(updated.started_at.is_none());
    }
}
