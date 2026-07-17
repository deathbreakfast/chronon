use std::time::Instant;

use anyhow::Result;
use chronon_testkit::{BootstrapSession, RunMode, ScenarioRunner, ScenarioSpec};

use crate::report::BenchReport;
use crate::runners::RunContext;
use crate::stats::MetricStats;

/// BM-CH0: measure coordinator tick latency with optional warmup.
pub async fn run(ctx: &RunContext) -> Result<BenchReport> {
    let spec = ScenarioSpec::scheduler_tick_smoke();
    let mut session = BootstrapSession::new(ctx.matrix.clone());
    session.install().await?;
    session.init_partitions().await?;

    let mut runner = ScenarioRunner::new(&mut session);
    if ctx.warmup > 0 {
        for _ in 0..ctx.warmup {
            let result = runner.run(&spec, RunMode::Benchmark).await?;
            if result.error.is_some() {
                anyhow::bail!("warmup failed: {:?}", result.error);
            }
        }
    }

    let mut tick_samples_ms = Vec::with_capacity(ctx.plan.default_ops);
    for _ in 0..ctx.plan.default_ops {
        let start = Instant::now();
        let tick = session.tick_once().await?;
        let _ = tick;
        tick_samples_ms.push(start.elapsed().as_secs_f64() * 1000.0);
    }

    let stats = MetricStats::summarize(tick_samples_ms);
    let mut report = BenchReport::base(&ctx.plan.id, &ctx.matrix);
    report.scenario_id = Some(spec.id);
    report.ops = Some(ctx.plan.default_ops);
    report.tick_ms = Some(stats);
    report.pass_notes = Some(format!(
        "tick p95 {:.3} ms over {} samples",
        stats.p95, stats.count
    ));
    Ok(report)
}
