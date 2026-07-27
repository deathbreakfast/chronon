//! Install [`ConsoleSink`] at boot so scheduler telemetry prints as `tracing` events —
//! **Integrating the host** (telemetry).
//!
//! ```bash
//! cargo run -p uf-chronon --example telemetry_console --features mem,telemetry-console
//! ```
//!
//! [`ConsoleSink`] writes through `tracing` on target `chronon_telemetry` (counters, gauges,
//! and structured events); a host `tracing_subscriber` renders them as console lines. Mirrors
//! Photon's `telemetry_ops_log` pattern, applied to Chronon's own scheduler/executor
//! self-telemetry rather than a host ops log.

use std::sync::Arc;

use chrono::{Duration, Utc};
use chronon::prelude::*;
use chronon_backend_mem::InMemorySchedulerStore;
use chronon_telemetry::ConsoleSink;

#[chronon::script(name = "telemetry_demo")]
#[allow(clippy::unused_async)] // script handlers are always async
async fn telemetry_demo(ctx: Box<dyn ScriptContext>) -> chronon::Result<()> {
    let _ = ctx.label();
    Ok(())
}

#[tokio::main]
async fn main() -> chronon::Result<()> {
    // `chronon_telemetry` target must be at info level (or above) to see ConsoleSink output.
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
        .telemetry_sink(Arc::new(ConsoleSink))
        .embedded()
        .auto_registry()
        .build()?;

    let mut job = Job::new("telemetry-demo-job", "telemetry_demo");
    job.schedule_kind = ScheduleKind::RunOnce;
    job.next_run_at = Some(Utc::now() - Duration::seconds(60));
    chronon.coordinator_service().upsert_job(job).await?;

    chronon.scheduler.init_partitions().await;
    // One tick emits `chronon_scheduler_ticks` plus enqueue events through ConsoleSink — look
    // for `target="chronon_telemetry"` lines above this process's final output.
    let tick = chronon.tick_once().await?;
    assert!(tick.enqueued >= 1, "expected at least one enqueued run");

    eprintln!(
        "telemetry_console: tick enqueued {} run(s) — see chronon_telemetry lines above",
        tick.enqueued
    );
    Ok(())
}
