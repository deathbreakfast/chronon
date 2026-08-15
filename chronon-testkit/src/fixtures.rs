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

/// Canonical panic probe script name (handler panics).
pub const PANIC_SCRIPT: &str = "testkit-panic";

/// Bounded sleep probe for burst-capacity benches (`params.sleep_ms`).
///
/// Accepts only a non-negative integer `sleep_ms` at or below [`MAX_SLEEP_MS`].
/// Malformed, negative, or over-limit values return [`chronon_core::ChrononError::ParamError`]
/// without sleeping.
///
/// ```no_run
/// # async fn demo(store: &dyn chronon_core::store::SchedulerStore) -> chronon_core::Result<()> {
/// use chronon_testkit::{upsert_immediate_cron_job, SLEEP_SCRIPT};
///
/// let mut job = upsert_immediate_cron_job(
///     store,
///     "midnight-0001",
///     SLEEP_SCRIPT,
///     "0 0 * * * *",
/// )
/// .await?;
/// job.params_json = serde_json::json!({ "sleep_ms": 250 });
/// store.upsert_job(&job).await?;
/// # Ok(())
/// # }
/// ```
pub const SLEEP_SCRIPT: &str = "testkit-sleep";

/// Upper bound for [`SLEEP_SCRIPT`] `sleep_ms` (milliseconds).
pub const MAX_SLEEP_MS: u64 = 5_000;

static COUNTING_RUNS: AtomicUsize = AtomicUsize::new(0);

/// Minimal actor JSON for [`chronon_core::JsonScriptContextFactory`].
pub fn smoke_actor_json() -> Value {
    json!({ "kind": "system", "operation": "testkit" })
}

/// Register built-in probe scripts on `registry`.
pub fn register_builtin_probes(registry: &mut ScriptRegistry) {
    registry.register(&ScriptDescriptor::new(NOOP_SCRIPT, noop_probe));
    registry.register(&ScriptDescriptor::new(COUNTING_SCRIPT, counting_probe));
    registry.register(&ScriptDescriptor::new(FAIL_SCRIPT, fail_probe));
    registry.register(&ScriptDescriptor::new(PANIC_SCRIPT, panic_probe));
    registry.register(&ScriptDescriptor::new(SLEEP_SCRIPT, sleep_probe));
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

fn panic_probe(
    _ctx: Box<dyn ScriptContext>,
    _params: Value,
) -> Pin<Box<dyn Future<Output = Result<()>> + Send>> {
    Box::pin(async {
        panic!("testkit panic probe");
    })
}

/// Parse and bound `sleep_ms` from script params.
///
/// # Errors
///
/// Returns [`chronon_core::ChrononError::ParamError`] when `sleep_ms` is missing, not an
/// integer, negative, or greater than [`MAX_SLEEP_MS`].
pub fn parse_sleep_ms(params: &Value) -> Result<u64> {
    let Some(raw) = params.get("sleep_ms") else {
        return Err(chronon_core::ChrononError::ParamError(
            "sleep_ms is required".into(),
        ));
    };
    let ms = match raw {
        Value::Number(n) => {
            if let Some(u) = n.as_u64() {
                u
            } else if let Some(i) = n.as_i64() {
                if i < 0 {
                    return Err(chronon_core::ChrononError::ParamError(
                        "sleep_ms must be non-negative".into(),
                    ));
                }
                i as u64
            } else {
                return Err(chronon_core::ChrononError::ParamError(
                    "sleep_ms must be an integer".into(),
                ));
            }
        }
        _ => {
            return Err(chronon_core::ChrononError::ParamError(
                "sleep_ms must be an integer".into(),
            ));
        }
    };
    if ms > MAX_SLEEP_MS {
        return Err(chronon_core::ChrononError::ParamError(format!(
            "sleep_ms {ms} exceeds max {MAX_SLEEP_MS}"
        )));
    }
    Ok(ms)
}

fn sleep_probe(
    _ctx: Box<dyn ScriptContext>,
    params: Value,
) -> Pin<Box<dyn Future<Output = Result<()>> + Send>> {
    Box::pin(async move {
        let ms = parse_sleep_ms(&params)?;
        if ms > 0 {
            sleep(TokioDuration::from_millis(ms)).await;
        }
        Ok(())
    })
}

/// Seed `count` due cron jobs with unique names (BM-CH1 / BM-CHL* workloads).
pub async fn seed_due_cron_jobs(
    store: &dyn SchedulerStore,
    count: usize,
    script_name: &str,
) -> chronon_core::Result<()> {
    for i in 0..count {
        let job_name = format!("bench-seed-{i}");
        let mut job =
            upsert_immediate_cron_job(store, &job_name, script_name, "0 * * * * *").await?;
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
    job.partition_hash = Some(chronon_scheduler::partition_hash_i64_for_job_id(
        &job.job_id,
    ));
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
    job.partition_hash = Some(chronon_scheduler::partition_hash_i64_for_job_id(
        &job.job_id,
    ));
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
    job.partition_hash = Some(chronon_scheduler::partition_hash_i64_for_job_id(
        &job.job_id,
    ));
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
    job.partition_hash = Some(chronon_scheduler::partition_hash_i64_for_job_id(
        &job.job_id,
    ));
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

#[cfg(test)]
mod tests {
    use super::*;
    use chronon_core::{ChrononError, ContextFactory, NoOpContextFactory};
    use serde_json::json;

    fn ctx() -> Box<dyn ScriptContext> {
        NoOpContextFactory.build(&json!({})).unwrap()
    }

    #[tokio::test]
    async fn sleep_probe_honors_sleep_ms() {
        let start = tokio::time::Instant::now();
        sleep_probe(ctx(), json!({ "sleep_ms": 50 })).await.unwrap();
        assert!(start.elapsed() >= TokioDuration::from_millis(40));
    }

    #[tokio::test]
    async fn sleep_probe_rejects_missing_sleep_ms() {
        let err = sleep_probe(ctx(), json!({})).await.unwrap_err();
        assert!(matches!(err, ChrononError::ParamError(_)));
    }

    #[tokio::test]
    async fn sleep_probe_rejects_negative() {
        let err = sleep_probe(ctx(), json!({ "sleep_ms": -1 }))
            .await
            .unwrap_err();
        assert!(matches!(err, ChrononError::ParamError(_)));
    }

    #[tokio::test]
    async fn sleep_probe_rejects_non_integer() {
        let err = sleep_probe(ctx(), json!({ "sleep_ms": 1.5 }))
            .await
            .unwrap_err();
        assert!(matches!(err, ChrononError::ParamError(_)));
        let err = sleep_probe(ctx(), json!({ "sleep_ms": "100" }))
            .await
            .unwrap_err();
        assert!(matches!(err, ChrononError::ParamError(_)));
    }

    #[tokio::test]
    async fn sleep_probe_rejects_over_limit_without_sleeping() {
        let start = tokio::time::Instant::now();
        let err = sleep_probe(ctx(), json!({ "sleep_ms": MAX_SLEEP_MS + 1 }))
            .await
            .unwrap_err();
        assert!(matches!(err, ChrononError::ParamError(_)));
        assert!(start.elapsed() < TokioDuration::from_millis(200));
    }

    #[test]
    fn parse_sleep_ms_accepts_zero() {
        assert_eq!(parse_sleep_ms(&json!({ "sleep_ms": 0 })).unwrap(), 0);
    }
}
