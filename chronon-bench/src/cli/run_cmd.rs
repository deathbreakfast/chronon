//! Single experiment run handler.

use std::fs;
use std::path::PathBuf;

use anyhow::Result;

use crate::cli::bench_config::{resolve_bench_config, BenchConfigOverrides};
use crate::cli::{resolve_hardware, RunArgs};
use crate::config::PoolLayout;
use crate::experiments::resolve_experiment;
use crate::matrix::matrix_from_cli;
use crate::report::BenchReport;
use crate::runners::{run_experiment, RunContext};

/// Parameters for a matrix batch experiment run.
pub struct BatchRunParams<'a> {
    /// Experiment id.
    pub experiment: &'a str,
    /// Storage adapter slug.
    pub storage: &'a str,
    /// Deployment shape slug.
    pub deployment: &'a str,
    /// Telemetry adapter slug.
    pub telemetry: &'a str,
    /// Topology slug.
    pub topology: &'a str,
    /// Hardware profile slug.
    pub hardware: &'a str,
    /// Warmup iterations.
    pub warmup: usize,
    /// Optional sweep overrides.
    pub overrides: BenchConfigOverrides,
    /// Optional explicit report path.
    pub report_path: Option<&'a PathBuf>,
}

fn bench_overrides_from_run(args: &RunArgs, is_ch7: bool) -> BenchConfigOverrides {
    BenchConfigOverrides {
        worker_count: args
            .worker_count
            .or_else(|| if is_ch7 { args.ops.map(|o| o as u32) } else { None }),
        job_count: args.jobs,
        partition_count: args.partitions,
        prefill_count: args
            .prefill
            .or_else(|| if is_ch7 { args.jobs.map(|j| j as u64) } else { None }),
        bench_client_index: args.bench_client_index,
        bench_client_count: args.bench_client_count,
        pool_count: args.pool_count,
        pool_layout: args.pool_layout.as_deref().map(|v| match v.to_ascii_lowercase().as_str() {
            "distinct" => PoolLayout::Distinct,
            _ => PoolLayout::Shared,
        }),
        worker_host_count: args.worker_hosts,
        storage_topology: args.storage_topology.clone(),
        tick_batch_limit: None,
    }
}

/// Execute one benchmark experiment and write JSON report.
pub async fn run_single(args: RunArgs) -> Result<()> {
    let hardware = resolve_hardware(args.hardware.clone());
    std::env::set_var("CHRONON_BENCH_HARDWARE", &hardware);

    let matrix = matrix_from_cli(
        &args.storage,
        &args.deployment,
        &args.telemetry,
        &args.topology,
    )?;
    let plan = resolve_experiment(&args.experiment, args.ops, args.jobs)?;
    let is_ch7 = args.experiment.eq_ignore_ascii_case("bm-ch7")
        || args.experiment.eq_ignore_ascii_case("bm-ch7d");
    let bench = resolve_bench_config(
        &args.experiment,
        bench_overrides_from_run(&args, is_ch7),
    );

    let ctx = RunContext {
        matrix: matrix.clone(),
        plan,
        warmup: args.warmup,
        bench,
    };

    let out = run_experiment(&ctx).await?;
    write_report(
        &out,
        &report_path(args.report.as_ref(), &ctx),
    )?;
    Ok(())
}

/// Run one experiment and optionally write to a specific path (matrix batch helper).
pub async fn run_batch(params: BatchRunParams<'_>) -> Result<BenchReport> {
    std::env::set_var("CHRONON_BENCH_HARDWARE", params.hardware);
    let matrix = matrix_from_cli(
        params.storage,
        params.deployment,
        params.telemetry,
        params.topology,
    )?;
    let plan = resolve_experiment(params.experiment, None, params.overrides.job_count)?;
    let bench = resolve_bench_config(params.experiment, params.overrides);
    let ctx = RunContext {
        matrix: matrix.clone(),
        plan,
        warmup: params.warmup,
        bench,
    };
    let out = run_experiment(&ctx).await?;
    if let Some(path) = params.report_path {
        write_report(&out, path)?;
    }
    Ok(out)
}

fn report_path(args_report: Option<&PathBuf>, ctx: &RunContext) -> PathBuf {
    args_report
        .cloned()
        .unwrap_or_else(|| BenchReport::default_report_path(&ctx.plan.id, &ctx.matrix))
}

fn write_report(report: &BenchReport, path: &PathBuf) -> Result<()> {
    let json = serde_json::to_string_pretty(report)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, &json)?;
    println!("wrote {}", path.display());
    Ok(())
}
