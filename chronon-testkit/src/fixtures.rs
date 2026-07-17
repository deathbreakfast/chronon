//! Synthetic actors and built-in script probes for matrix scenarios.

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use chrono::{Duration, Utc};
use chronon_core::models::{Job, RunStatus, ScheduleKind};
use chronon_core::store::SchedulerStore;
use chronon_core::{Result, ScriptContext};
use chronon_executor::{ScriptDescriptor, ScriptRegistry};
use chronon_scheduler::CronExpr;
use serde_json::{json, Value};
use tokio::time::{sleep, Duration as TokioDuration};

/// Canonical noop probe script name (static for registry descriptors).
pub const NOOP_SCRIPT: &str = "testkit-noop";

/// Canonical counting probe script name.
pub const COUNTING_SCRIPT: &str = "testkit-counting";

/// Canonical failing probe script name.
pub const FAIL_SCRIPT: &str = "testkit-fail";

static COUNTING_RUNS: AtomicUsize = AtomicUsize::new(0);

/// Minimal actor JSON for [`chronon_core::JsonScriptContextFactory`].
pub fn smoke_actor_json() -> Value {
    json!({ "kind": "system", "operation": "testkit" })
}

/// Register built-in probe scripts on `registry`.
pub fn register_builtin_probes(registry: &mut ScriptRegistry) {
    registry.register(ScriptDescriptor::new(NOOP_SCRIPT, noop_probe));
    registry.register(ScriptDescriptor::new(COUNTING_SCRIPT, counting_probe));
    registry.register(ScriptDescriptor::new(FAIL_SCRIPT, fail_probe));
}

/// Reset the counting probe global (call at scenario start when needed).
pub fn reset_counting_probe() {
    COUNTING_RUNS.store(0, Ordering::SeqCst);
}

/// Return the global counting probe invocation total.
pub fn counting_probe_total() -> usize {
    COUNTING_RUNS.load(Ordering::SeqCst)
}

fn noop_probe(
    _ctx: Box<dyn ScriptContext>,
    _params: Value,
) -> Pin<Box<dyn Future<Output = Result<()>> + Send>> {
    Box::pin(async { Ok(()) })
}

fn counting_probe(
    _ctx: Box<dyn ScriptContext>,
    _params: Value,
) -> Pin<Box<dyn Future<Output = Result<()>> + Send>> {
    Box::pin(async {
        COUNTING_RUNS.fetch_add(1, Ordering::SeqCst);
        Ok(())
    })
}

fn fail_probe(
    _ctx: Box<dyn ScriptContext>,
    _params: Value,
) -> Pin<Box<dyn Future<Output = Result<()>> + Send>> {
    Box::pin(async { Err(chronon_core::ChrononError::Internal("probe failure".into())) })
}

/// Seed `count` due cron jobs with unique names (BM-CH1 / BM-CHL* workloads).
pub async fn seed_due_cron_jobs(
    store: &dyn SchedulerStore,
    count: usize,
    script_name: &str,
) -> chronon_core::Result<()> {
    for i in 0..count {
        let job_name = format!("bench-seed-{i}");
        let mut job = upsert_immediate_cron_job(store, &job_name, script_name, "0 * * * * *").await?;
        job.actor_json = smoke_actor_json();
        store.upsert_job(&job).await?;
    }
    Ok(())
}

/// Upsert a cron job due immediately (next_run_at in the past).
pub async fn upsert_immediate_cron_job(
    store: &dyn SchedulerStore,
    job_name: &str,
    script_name: &str,
    cron_expr: &str,
) -> chronon_core::Result<Job> {
    let mut job = Job::new(job_name, script_name);
    job.schedule_kind = ScheduleKind::Cron;
    job.cron_expr = Some(cron_expr.to_string());
    job.next_run_at = Some(Utc::now() - Duration::seconds(60));
    job.partition_hash = Some(chronon_scheduler::partition_hash_i64_for_job_id(&job.job_id));
    let _ = CronExpr::parse(cron_expr, None);
    store.upsert_job(&job).await?;
    Ok(job)
}

/// Upsert a cron job whose next fire time is in the future (not due on tick).
pub async fn upsert_future_cron_job(
    store: &dyn SchedulerStore,
    job_name: &str,
    script_name: &str,
    cron_expr: &str,
) -> chronon_core::Result<Job> {
    let mut job = Job::new(job_name, script_name);
    job.schedule_kind = ScheduleKind::Cron;
    job.cron_expr = Some(cron_expr.to_string());
    job.next_run_at = Some(Utc::now() + Duration::hours(1));
    job.partition_hash = Some(chronon_scheduler::partition_hash_i64_for_job_id(&job.job_id));
    store.upsert_job(&job).await?;
    Ok(job)
}

/// Upsert a manual job (only run via `run_now`).
pub async fn upsert_manual_job(
    store: &dyn SchedulerStore,
    job_name: &str,
    script_name: &str,
) -> chronon_core::Result<Job> {
    let mut job = Job::new(job_name, script_name);
    job.schedule_kind = ScheduleKind::Manual;
    job.next_run_at = None;
    job.partition_hash = Some(chronon_scheduler::partition_hash_i64_for_job_id(&job.job_id));
    store.upsert_job(&job).await?;
    Ok(job)
}

/// Upsert a run-once job due immediately.
pub async fn upsert_immediate_run_once_job(
    store: &dyn SchedulerStore,
    job_name: &str,
    script_name: &str,
) -> chronon_core::Result<Job> {
    let mut job = Job::new(job_name, script_name);
    job.schedule_kind = ScheduleKind::RunOnce;
    job.next_run_at = Some(Utc::now() - Duration::seconds(60));
    job.partition_hash = Some(chronon_scheduler::partition_hash_i64_for_job_id(&job.job_id));
    store.upsert_job(&job).await?;
    Ok(job)
}

/// Poll until a run for `job_name` reaches `status` or timeout.
pub async fn wait_for_run_terminal(
    store: Arc<dyn SchedulerStore>,
    job_name: &str,
    status: RunStatus,
    timeout: TokioDuration,
) -> chronon_core::Result<()> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        if tokio::time::Instant::now() >= deadline {
            return Err(chronon_core::ChrononError::Internal(format!(
                "timeout waiting for run {job_name} -> {status}"
            )));
        }
        if let Some(job) = store.get_job_by_name(job_name).await? {
            let runs = store.list_runs_for_job(&job.job_id, 100).await?;
            if runs.iter().any(|r| r.status == status) {
                return Ok(());
            }
        }
        sleep(TokioDuration::from_millis(50)).await;
    }
}

/// Count terminal runs for a job name.
pub async fn count_runs_for_job(
    store: &dyn SchedulerStore,
    job_name: &str,
) -> chronon_core::Result<usize> {
    let Some(job) = store.get_job_by_name(job_name).await? else {
        return Ok(0);
    };
    let runs = store.list_runs_for_job(&job.job_id, 100).await?;
    Ok(runs.len())
}
