use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use chronon_core::models::RunStatus;
use chronon_testkit::{
    upsert_immediate_run_once_job, wait_for_run_terminal, BootstrapSession, NOOP_SCRIPT,
};

use crate::report::BenchReport;
use crate::runners::RunContext;

/// BM-CH5: measure end-to-end successful script runs per second vs tokio spawn baseline.
pub async fn run(ctx: &RunContext) -> Result<BenchReport> {
    let ops = ctx.plan.default_ops.max(1);

    // Tokio spawn baseline: measure raw task spawn + join overhead.
    let baseline_start = Instant::now();
    let mut baseline_handles = Vec::with_capacity(ops);
    for _ in 0..ops {
        baseline_handles.push(tokio::spawn(async {}));
    }
    for handle in baseline_handles {
        let _ = handle.await;
    }
    let baseline_elapsed = baseline_start.elapsed().as_secs_f64().max(1e-9);
    let tokio_baseline_rps = ops as f64 / baseline_elapsed;

    let mut session = BootstrapSession::new(ctx.matrix.clone());
    session.install().await?;
    session.spawn_embedded().await?;

    let store = session.store_dyn()?;
    let start = Instant::now();
    let mut successes = 0usize;

    for i in 0..ops {
        let job_name = format!("bench-ch5-{i}");
        let mut job = upsert_immediate_run_once_job(store.as_ref(), &job_name, NOOP_SCRIPT).await?;
        job.actor_json = chronon_testkit::smoke_actor_json();
        store.upsert_job(&job).await?;

        wait_for_run_terminal(
            Arc::clone(&store),
            &job_name,
            RunStatus::Success,
            std::time::Duration::from_secs(5),
        )
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
        successes += 1;
    }

    session.shutdown_embedded().await?;

    let elapsed = start.elapsed().as_secs_f64().max(1e-9);
    let runs_per_sec = successes as f64 / elapsed;

    let mut report = BenchReport::base(&ctx.plan.id, &ctx.matrix);
    report.ops = Some(ops);
    report.runs_per_sec = Some(runs_per_sec);
    report.tokio_spawn_baseline_runs_per_sec = Some(tokio_baseline_rps);
    report.error_rate = Some(1.0 - (successes as f64 / ops as f64));
    report.pass_notes = Some(format!(
        "{runs_per_sec:.2} successful runs/s over {ops} jobs (tokio baseline {tokio_baseline_rps:.0} spawns/s)"
    ));
    Ok(report)
}
