//! A script dispatched by the worker sees its own run through `ScriptContext::run`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use chronon_backend_mem::InMemorySchedulerStore;
use chronon_core::models::RunStatus;
use chronon_core::store::SchedulerStore;
use chronon_core::{Job, RunTrigger, ScheduleKind, ScriptContext};
use chronon_executor::{ScriptDescriptor, ScriptRegistry};
use chronon_runtime::{ChrononBuilder, CoordinatorService};

/// `(run id, attempt, job id, trigger, started_at set)` as the probe script saw them.
type Seen = (String, u32, String, RunTrigger, bool);

static SEEN: Mutex<Vec<Seen>> = Mutex::new(Vec::new());

// The signature is fixed by `InvokeFn`.
#[allow(clippy::needless_pass_by_value)]
fn probe(
    ctx: Box<dyn ScriptContext>,
    _params: serde_json::Value,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = chronon_core::Result<()>> + Send>> {
    let values = ctx.run();
    SEEN.lock().unwrap().push((
        values.run.id.clone(),
        values.run.attempt,
        values.job.id.clone(),
        values.run.trigger,
        values.run.started_at.is_some(),
    ));
    Box::pin(async { Ok(()) })
}

#[tokio::test]
async fn worker_hands_script_its_run_values() {
    let store: Arc<dyn SchedulerStore> = Arc::new(InMemorySchedulerStore::new());
    let mut registry = ScriptRegistry::new();
    registry.register(&ScriptDescriptor::new("run_scope_probe", probe));

    let coordinator = CoordinatorService::new(Arc::clone(&store));
    let mut job = Job::new("run-scope-job", "run_scope_probe");
    job.schedule_kind = ScheduleKind::Manual;
    let job_id = job.job_id.clone();
    coordinator.upsert_job(job).await.unwrap();
    let run_id = coordinator.run_now(&job_id).await.unwrap();

    let mut chronon = ChrononBuilder::new()
        .scheduler_store(Arc::clone(&store))
        .script_registry(Arc::new(registry))
        .worker("general")
        .build()
        .expect("build worker");
    let shutdown = chronon.shutdown_handle();
    tokio::spawn(async move {
        let _ = chronon.run().await;
    });

    let started = tokio::time::Instant::now();
    loop {
        let run = store.get_run(&run_id).await.unwrap().expect("run");
        if run.status.is_terminal() {
            assert_eq!(run.status, RunStatus::Success, "{:?}", run.error_json);
            break;
        }
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "worker did not finish the run in time"
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    shutdown.notify_waiters();

    let seen = SEEN.lock().unwrap().clone();
    assert_eq!(
        seen,
        vec![(run_id, 1, job_id, RunTrigger::Manual, true)],
        "script should see its own run values"
    );
}
