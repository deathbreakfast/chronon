use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use chronon_core::models::RunStatus;
use chronon_testkit::{
    upsert_immediate_run_once_job, wait_for_run_terminal, BootstrapSession, DeploymentKind,
    MatrixSpec, NOOP_SCRIPT,
};

use crate::report::BenchReport;
use crate::runners::RunContext;
use crate::stats::MetricStats;

async fn measure_enqueue_to_run(matrix: MatrixSpec, ops: usize) -> Result<Vec<f64>> {
    let mut session = BootstrapSession::new(matrix);
    session.install().await?;
    session.spawn_embedded().await?;

    let store = session.store_dyn()?;
    let mut samples = Vec::with_capacity(ops);

    for i in 0..ops {
        let job_name = format!("bench-ch6-{i}");
        let start = Instant::now();
        let mut job =
            upsert_immediate_run_once_job(store.as_ref(), &job_name, NOOP_SCRIPT).await?;
        job.actor_json = chronon_testkit::smoke_actor_json();
        store.upsert_job(&job).await?;

        wait_for_run_terminal(
            Arc::clone(&store),
            &job_name,
            RunStatus::Success,
            std::time::Duration::from_secs(5),
        )
        .await
        .map_err(|e| anyhow::anyhow!("ch6 iteration {i}: {e}"))?;
        samples.push(start.elapsed().as_secs_f64() * 1000.0);
    }

    session.shutdown_embedded().await?;
    Ok(samples)
}

/// BM-CH6: compare enqueue-to-run latency for embedded vs coordinator-worker deployment.
pub async fn run(ctx: &RunContext) -> Result<BenchReport> {
    let ops = ctx.plan.default_ops.max(1);

    let embedded_matrix = MatrixSpec {
        deployment: DeploymentKind::Embedded,
        ..ctx.matrix.clone()
    };
    let split_matrix = MatrixSpec {
        deployment: DeploymentKind::CoordinatorWorker,
        ..ctx.matrix.clone()
    };

    let embedded_samples = measure_enqueue_to_run(embedded_matrix, ops).await?;
    let split_samples = measure_enqueue_to_run(split_matrix, ops).await?;

    let embedded_stats = MetricStats::summarize(embedded_samples);
    let split_stats = MetricStats::summarize(split_samples);
    let delta = split_stats.p95 - embedded_stats.p95;

    let mut report = BenchReport::base(&ctx.plan.id, &ctx.matrix);
    report.ops = Some(ops);
    report.embedded_enqueue_to_run_ms = Some(embedded_stats);
    report.enqueue_to_run_ms = Some(split_stats);
    report.pass_notes = Some(format!(
        "embedded p95 {:.3} ms vs coordinator-worker p95 {:.3} ms (delta {:.3} ms)",
        embedded_stats.p95, split_stats.p95, delta
    ));
    Ok(report)
}
