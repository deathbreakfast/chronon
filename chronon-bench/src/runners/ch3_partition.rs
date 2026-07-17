use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use chronon_scheduler::PartitionAssigner;
use chronon_telemetry::NoOpSink;
use chronon_testkit::BootstrapSession;

use crate::report::BenchReport;
use crate::runners::RunContext;
use crate::stats::MetricStats;

/// BM-CH3: measure partition reassignment and tick delay during lease churn.
pub async fn run(ctx: &RunContext) -> Result<BenchReport> {
    let partitions_n = ctx.bench.partition_count;

    let mut session = BootstrapSession::new(ctx.matrix.clone()).with_num_partitions(partitions_n);
    session.install().await?;
    session.init_partitions().await?;

    let store = session.store_dyn()?;
    let telemetry: Arc<dyn chronon_telemetry::TelemetrySink> = Arc::new(NoOpSink);
    let assigner = Arc::new(PartitionAssigner::new(
        Arc::clone(&store),
        telemetry,
        "bench-coord-a".into(),
        partitions_n,
    ));

    let mut reassign_samples = Vec::with_capacity(ctx.plan.default_ops);
    let mut tick_delay_samples = Vec::with_capacity(ctx.plan.default_ops);

    for _ in 0..ctx.plan.default_ops {
        let start = Instant::now();
        assigner.refresh_leases().await?;
        reassign_samples.push(start.elapsed().as_secs_f64() * 1000.0);

        let tick_start = Instant::now();
        let _ = session.tick_once().await?;
        tick_delay_samples.push(tick_start.elapsed().as_secs_f64() * 1000.0);
    }

    let reassign = MetricStats::summarize(reassign_samples);
    let tick_delay = MetricStats::summarize(tick_delay_samples);
    let mut report = BenchReport::base(&ctx.plan.id, &ctx.matrix);
    report.ops = Some(ctx.plan.default_ops);
    report.reassign_ms = Some(reassign);
    report.tick_delay_ms = Some(tick_delay);
    report.pass_notes = Some(format!(
        "reassign p95 {:.3} ms; tick delay p95 {:.3} ms during churn ({partitions_n} partitions)",
        reassign.p95, tick_delay.p95
    ));
    Ok(report)
}
