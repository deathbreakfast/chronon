//! Schedule a due cron job, tick once, and verify a run is enqueued.
//!
//! ```bash
//! cargo run -p uf-chronon --example embedded_tick --features mem
//! ```

use std::sync::Arc;

use chrono::{Duration, Utc};
use chronon::prelude::*;
use chronon_backend_mem::InMemorySchedulerStore;
use chronon_core::models::ScheduleKind;

#[chronon::script(name = "tick_demo")]
async fn tick_demo(ctx: Box<dyn ScriptContext>) -> chronon::Result<()> {
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

    let mut job = Job::new("tick-demo-job", "tick_demo");
    job.schedule_kind = ScheduleKind::RunOnce;
    job.next_run_at = Some(Utc::now() - Duration::seconds(60));
    chronon.coordinator_service().upsert_job(job).await?;

    chronon.scheduler.init_partitions().await;
    let tick = chronon.tick_once().await?;
    assert!(tick.enqueued >= 1, "expected at least one enqueued run");

    let jobs = store.list_jobs().await?;
    let job_id = jobs[0].job_id.clone();
    let runs = store.list_runs_for_job(&job_id, 10).await?;
    assert!(!runs.is_empty(), "expected a persisted run row");

    eprintln!("tick enqueued {} run(s)", tick.enqueued);
    Ok(())
}
