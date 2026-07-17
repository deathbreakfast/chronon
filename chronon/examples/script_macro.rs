//! Define a script with `#[chronon::script]`, auto-discover via Quark inventory, schedule a job, and tick.
//!
//! Run: `cargo run -p uf-chronon --example script_macro --features mem`

use std::sync::Arc;

use chrono::{Duration, Utc};
use chronon::prelude::*;
use chronon_backend_mem::InMemorySchedulerStore;

#[chronon::script(name = "nightly_cleanup")]
async fn nightly_cleanup(
    ctx: Box<dyn ScriptContext>,
    retention_days: u32,
) -> chronon::Result<()> {
    let _ = (ctx.label(), retention_days);
    Ok(())
}

#[tokio::main]
async fn main() -> chronon::Result<()> {
    let store = Arc::new(InMemorySchedulerStore::new());
    let chronon = ChrononBuilder::new()
        .scheduler_store(store)
        .context_factory(Arc::new(JsonScriptContextFactory))
        .embedded()
        .auto_registry()
        .build()?;

    assert!(chronon.executor().script_count() >= 1);

    let mut job = Job::new("nightly-job", "nightly_cleanup");
    job.schedule_kind = ScheduleKind::RunOnce;
    job.params_json = serde_json::json!({ "retention_days": 7 });
    job.next_run_at = Some(Utc::now() - Duration::seconds(60));
    chronon.coordinator_service().upsert_job(job).await?;

    chronon.scheduler.init_partitions().await;
    let tick = chronon.tick_once().await?;
    assert!(tick.enqueued >= 1);

    println!(
        "script registered; tick enqueued {} run(s)",
        tick.enqueued
    );
    Ok(())
}
