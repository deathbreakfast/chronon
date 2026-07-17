use std::time::Instant;

use anyhow::Result;
use chrono::Utc;
use chronon_scheduler::CronExpr;

use crate::report::BenchReport;
use crate::runners::RunContext;

const SAMPLE_EXPR: &str = "0 12 * * *";

/// BM-CH2: compare Chronon cron evaluation throughput against croner baseline.
pub async fn run(ctx: &RunContext) -> Result<BenchReport> {
    let ops = ctx.plan.default_ops;
    let cron = CronExpr::parse(SAMPLE_EXPR, None)?;
    let baseline = croner::Cron::new(SAMPLE_EXPR)
        .with_seconds_optional()
        .parse()
        .map_err(|e| anyhow::anyhow!("croner parse: {e}"))?;
    let base = Utc::now();

    let start = Instant::now();
    for _ in 0..ops {
        let _ = cron.next_after(base);
    }
    let elapsed = start.elapsed().as_secs_f64();
    let evals_per_sec = ops as f64 / elapsed.max(1e-9);

    let start = Instant::now();
    let after = base.with_timezone(&chrono_tz::Tz::UTC);
    for _ in 0..ops {
        let _ = baseline
            .find_next_occurrence(&after, false)
            .ok();
    }
    let baseline_elapsed = start.elapsed().as_secs_f64();
    let baseline_evals = ops as f64 / baseline_elapsed.max(1e-9);

    let mut report = BenchReport::base(&ctx.plan.id, &ctx.matrix);
    report.ops = Some(ops);
    report.cron_evals_per_sec = Some(evals_per_sec);
    report.cron_baseline_evals_per_sec = Some(baseline_evals);
    report.pass_notes = Some(format!(
        "CronExpr {evals_per_sec:.0} evals/s vs croner {baseline_evals:.0} evals/s"
    ));
    Ok(report)
}
