use std::time::Instant;

use anyhow::Result;
use chronon_testkit::{seed_due_cron_jobs, BootstrapSession, NOOP_SCRIPT};

use crate::report::BenchReport;
use crate::runners::RunContext;
use crate::stats::MetricStats;

/// BM-CHL*: sustained tick load with many due jobs (id selects job count tier).
pub async fn run(ctx: &RunContext) -> Result<BenchReport> {
    let jobs = ctx.bench.job_count;
    let mut session = BootstrapSession::new(ctx.matrix.clone());
    session.install().await?;
    session.init_partitions().await?;

    let store = session.store_dyn()?;
    seed_due_cron_jobs(store.as_ref(), jobs, NOOP_SCRIPT).await?;

    let mut tick_samples = Vec::with_capacity(ctx.plan.default_ops);
    let mut errors = 0usize;

    for _ in 0..ctx.plan.default_ops {
        let start = Instant::now();
        match session.tick_once().await {
            Ok(_tick) => {
                tick_samples.push(start.elapsed().as_secs_f64() * 1000.0);
            }
            Err(_) => errors += 1,
        }
    }

    let stats = MetricStats::summarize(tick_samples);
    let error_rate = errors as f64 / ctx.plan.default_ops as f64;

    let mut report = BenchReport::base(&ctx.plan.id, &ctx.matrix);
    report.jobs = Some(jobs);
    report.ops = Some(ctx.plan.default_ops);
    report.tick_ms = Some(stats);
    report.error_rate = Some(error_rate);
    report.status = if error_rate < 0.001 {
        "ok".to_string()
    } else {
        "fail".to_string()
    };
    report.pass_notes = Some(format!(
        "sustained tick p99 {:.3} ms with {jobs} due jobs/tick; error rate {:.4}",
        stats.p99, error_rate
    ));
    Ok(report)
}
