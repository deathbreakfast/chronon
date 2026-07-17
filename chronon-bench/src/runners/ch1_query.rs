use std::time::Instant;

use anyhow::Result;
use chrono::{Duration, Utc};
use chronon_testkit::{seed_due_cron_jobs, BootstrapSession, NOOP_SCRIPT};

use crate::report::BenchReport;
use crate::runners::RunContext;
use crate::stats::MetricStats;

/// BM-CH1: measure due-job query latency with a seeded job set.
pub async fn run(ctx: &RunContext) -> Result<BenchReport> {
    let jobs = ctx.bench.job_count;
    let partitions_n = ctx.bench.partition_count;
    let batch_limit = ctx.bench.tick_batch_limit;

    let mut session = BootstrapSession::new(ctx.matrix.clone()).with_num_partitions(partitions_n);
    session.install().await?;
    session.init_partitions().await?;

    let store = session.store_dyn()?;
    seed_due_cron_jobs(store.as_ref(), jobs, NOOP_SCRIPT).await?;

    let partitions: Vec<u32> = (0..partitions_n).collect();
    let until = Utc::now() + Duration::seconds(3600);

    let mut query_samples_ms = Vec::with_capacity(ctx.plan.default_ops);
    for _ in 0..ctx.plan.default_ops {
        let start = Instant::now();
        let _due = store
            .find_due_job_ids_in_partitions(&partitions, until, batch_limit)
            .await?;
        query_samples_ms.push(start.elapsed().as_secs_f64() * 1000.0);
    }

    let stats = MetricStats::summarize(query_samples_ms);
    let mut report = BenchReport::base(&ctx.plan.id, &ctx.matrix);
    report.jobs = Some(jobs);
    report.ops = Some(ctx.plan.default_ops);
    report.query_ms = Some(stats);
    report.pass_notes = Some(format!(
        "due query p95 {:.3} ms with {jobs} seeded jobs across {partitions_n} partitions",
        stats.p95
    ));
    Ok(report)
}
