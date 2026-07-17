//! Per-experiment benchmark runners.

mod ch0_tick;
mod ch1_query;
mod ch2_cron;
mod ch3_partition;
mod ch4_leader;
mod ch5_script;
mod ch6_deploy;
mod ch7_claim;
pub mod ch7_common;
mod ch7_daemon;
mod chl_load;

use anyhow::Result;
use chronon_testkit::MatrixSpec;

use crate::config::BenchRunConfig;
use crate::experiments::ExperimentPlan;
use crate::pass_eval::evaluate_verdict;
use crate::report::BenchReport;

/// Inputs shared by all experiment runners.
pub struct RunContext {
    /// Matrix row under test.
    pub matrix: MatrixSpec,
    /// Resolved experiment plan (id, ops, jobs).
    pub plan: ExperimentPlan,
    /// Warmup iterations before measured samples (tick experiments).
    pub warmup: usize,
    /// Sweep knobs for this run.
    pub bench: BenchRunConfig,
}

/// Attach sweep dimensions and optional verdict to a finished report.
pub fn finalize_report(mut report: BenchReport, bench: &BenchRunConfig) -> BenchReport {
    report.sweep_dimensions = Some(bench.sweep_dimensions());
    report.verdict = evaluate_verdict(&report);
    report
}

/// Dispatch to the runner for `ctx.plan.id`.
pub async fn run_experiment(ctx: &RunContext) -> Result<BenchReport> {
    let report = match ctx.plan.id.as_str() {
        "bm-ch0" => ch0_tick::run(ctx).await?,
        "bm-ch1" => ch1_query::run(ctx).await?,
        "bm-ch2" => ch2_cron::run(ctx).await?,
        "bm-ch3" => ch3_partition::run(ctx).await?,
        "bm-ch4" => ch4_leader::run(ctx).await?,
        "bm-ch5" => ch5_script::run(ctx).await?,
        "bm-ch6" => ch6_deploy::run(ctx).await?,
        "bm-ch7" => ch7_claim::run(ctx).await?,
        "bm-ch7d" => ch7_daemon::run(ctx).await?,
        id if id.starts_with("bm-chl") => chl_load::run(ctx).await?,
        other => anyhow::bail!("no runner for {other}"),
    };
    Ok(finalize_report(report, &ctx.bench))
}
