//! BM-CH-RETRY: fail → schedule-next retry latency (mem store).

use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use chronon_backend_mem::InMemorySchedulerStore;
use chronon_core::models::{Job, RetryPolicy, Run, RunStatus};
use chronon_core::store::SchedulerStore;
use chronon_runtime::finalize_failed_run;

use crate::report::BenchReport;
use crate::runners::RunContext;
use crate::stats::MetricStats;

/// Measure wall time of [`finalize_failed_run`] when a retry is enqueued.
pub async fn run(ctx: &RunContext) -> Result<BenchReport> {
    let ops = ctx.plan.default_ops.max(1);
    let store: Arc<dyn SchedulerStore> = Arc::new(InMemorySchedulerStore::new());

    let mut samples = Vec::with_capacity(ops);
    for i in 0..ops {
        let mut job = Job::new(format!("bench-retry-{i}"), "noop");
        job.set_retry_policy(&RetryPolicy {
            max_attempts: 3,
            base_delay_ms: 0,
            backoff_multiplier: 1.0,
            max_delay_ms: 0,
        });
        store.upsert_job(&job).await?;
        let run = Run::for_job(&job.job_id, "noop", chrono::Utc::now());
        store.create_run(&run).await?;

        let start = Instant::now();
        finalize_failed_run(&store, run, &job, RunStatus::Failed, "bench", None).await;
        samples.push(start.elapsed().as_secs_f64() * 1000.0);
    }

    let mut report = BenchReport::base(&ctx.plan.id, &ctx.matrix);
    report.ops = Some(ops);
    report.enqueue_to_run_ms = Some(MetricStats::summarize(samples));
    report.pass_notes = Some(format!(
        "finalize_failed_run retry enqueue latency over {ops} ops (mem store)"
    ));
    report.error_rate = Some(0.0);
    Ok(report)
}
