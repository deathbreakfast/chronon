//! Define a script with `#[chronon::script]`, auto-discover via Quark inventory, schedule a job, and tick.
//!
//! Run: `cargo run -p uf-chronon --example script_macro --features mem`

use std::sync::Arc;

use chrono::{Duration, Utc};
use chronon::prelude::*;
use chronon_backend_mem::InMemorySchedulerStore;

#[chronon::script(name = "nightly_cleanup")]
#[allow(clippy::unused_async)] // script handlers are always async
async fn nightly_cleanup(ctx: Box<dyn ScriptContext>, retention_days: u32) -> chronon::Result<()> {
    let _ = (ctx.label(), retention_days);
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
        .scheduler_store(store)
        .context_factory(Arc::new(JsonScriptContextFactory))
        .embedded()
        .auto_registry()
        .build()?;

    assert!(chronon.executor().script_count() >= 1);

    let job = JobBuilder::new(&nightly_cleanup())
        .name("nightly-job")
        .run_once_at(Utc::now() - Duration::seconds(60))
        .params(NightlyCleanupParams { retention_days: 7 })
        .build()?;
    chronon.coordinator_service().upsert_job(job).await?;

    chronon.scheduler.init_partitions().await;
    let tick = chronon.tick_once().await?;
    assert!(tick.enqueued >= 1);

    eprintln!("script registered; tick enqueued {} run(s)", tick.enqueued);
    Ok(())
}
