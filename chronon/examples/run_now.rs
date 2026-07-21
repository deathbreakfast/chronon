//! Trigger an immediate run with `CoordinatorService::run_now`.
//!
//! Manual jobs are never due for the tick loop — they only enqueue via `run_now`
//! (or HTTP `POST /jobs/run_now`). This example upserts a manual job, triggers it,
//! and asserts a queued run row exists.
//!
//! Run: `cargo run -p uf-chronon --example run_now --features mem`

use std::sync::Arc;

use chronon::prelude::*;
use chronon_backend_mem::InMemorySchedulerStore;

#[chronon::script(name = "manual_probe")]
async fn manual_probe(ctx: Box<dyn ScriptContext>) -> chronon::Result<()> {
    let _ = ctx.label();
    Ok(())
}

#[tokio::main]
async fn main() -> chronon::Result<()> {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .try_init();

    let store = Arc::new(InMemorySchedulerStore::new());
    let chronon = ChrononBuilder::new()
        .scheduler_store(store.clone())
        .context_factory(Arc::new(JsonScriptContextFactory))
        .embedded()
        .auto_registry()
        .build()?;

    let mut job = Job::new("manual-job", "manual_probe");
    job.schedule_kind = ScheduleKind::Manual;
    chronon
        .coordinator_service()
        .upsert_job(job.clone())
        .await?;

    let run_id = chronon.coordinator_service().run_now(&job.job_id).await?;
    let run = store
        .get_run(&run_id)
        .await?
        .ok_or_else(|| chronon::ChrononError::Internal("run_now should persist a run".into()))?;
    assert_eq!(run.status, RunStatus::Queued);
    assert_eq!(run.job_id.as_deref(), Some(job.job_id.as_str()));

    eprintln!("run_now enqueued run_id={run_id}");
    Ok(())
}
