//! CLI subcommands for `chronon-bench`.

mod aggregate_cmd;
pub mod bench_config;
mod matrix_cmd;
mod run_cmd;
mod scaling_curve_cmd;

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::experiments::ALL_EXPERIMENT_IDS;

/// `chronon-bench` top-level CLI.
#[derive(Parser)]
#[command(name = "chronon-bench", about = "Chronon synthetic benchmark runner")]
pub struct Cli {
    /// Selected subcommand.
    #[command(subcommand)]
    pub command: Command,
}

/// Subcommands.
#[derive(Subcommand)]
pub enum Command {
    /// List registered experiment IDs (see EXPERIMENTS.md).
    Experiments,
    /// Run one experiment id against a matrix slice.
    Run(RunArgs),
    /// Run every experiment in a named matrix slice.
    Matrix(MatrixArgs),
    /// Project scaling curve from report JSONs.
    ScalingCurve(ScalingCurveArgs),
    /// Aggregate multibench BM-CH7 per-client reports into fleet totals.
    Aggregate(AggregateArgs),
}

/// Arguments for `aggregate` subcommand.
#[derive(Parser)]
pub struct AggregateArgs {
    /// Storage adapter slug to filter reports.
    #[arg(long, default_value = "postgres-redis")]
    pub storage: String,
    /// Hardware profile slug to filter reports.
    #[arg(long)]
    pub hardware: Option<String>,
    /// Directory containing benchmark report JSONs.
    #[arg(long, default_value = "profiling/chronon-bench/reports")]
    pub reports_dir: PathBuf,
    /// Filename prefix for multibench cell reports.
    #[arg(long)]
    pub cell_prefix: String,
}

/// Arguments for `run` subcommand.
#[derive(Parser)]
pub struct RunArgs {
    /// Experiment id (for example `bm-ch0`).
    #[arg(long)]
    pub experiment: String,
    /// Storage adapter slug.
    #[arg(long, default_value = "mem")]
    pub storage: String,
    /// Deployment shape slug.
    #[arg(long, default_value = "embedded")]
    pub deployment: String,
    /// Telemetry adapter slug.
    #[arg(long, default_value = "off")]
    pub telemetry: String,
    /// Topology slug.
    #[arg(long, default_value = "isolated-lab")]
    pub topology: String,
    /// Measured iteration count (or worker count for legacy `bm-ch7` when `--worker-count` omitted).
    #[arg(long)]
    pub ops: Option<usize>,
    /// Seeded job count (S1 / CHL tiers).
    #[arg(long)]
    pub jobs: Option<usize>,
    /// Parallel worker count for BM-CH7 (S0).
    #[arg(long)]
    pub worker_count: Option<u32>,
    /// Partition count for BM-CH1/CH3 (S3).
    #[arg(long)]
    pub partitions: Option<u32>,
    /// Prefill run count for BM-CH7 (S4).
    #[arg(long)]
    pub prefill: Option<u64>,
    /// Multibench client index (S5 / D3).
    #[arg(long)]
    pub bench_client_index: Option<u32>,
    /// Multibench client count (S5 / D3).
    #[arg(long)]
    pub bench_client_count: Option<u32>,
    /// Worker pool count for BM-CH7 D1.
    #[arg(long)]
    pub pool_count: Option<u32>,
    /// Pool layout: `shared` or `distinct`.
    #[arg(long)]
    pub pool_layout: Option<String>,
    /// Worker daemon host count for BM-CH7D D4.
    #[arg(long)]
    pub worker_hosts: Option<u32>,
    /// Data-tier topology label (D2).
    #[arg(long)]
    pub storage_topology: Option<String>,
    /// Hardware profile slug written into reports.
    #[arg(long)]
    pub hardware: Option<String>,
    /// Warmup ticks before measured samples.
    #[arg(long, default_value_t = 0)]
    pub warmup: usize,
    /// Optional explicit JSON report path.
    #[arg(long)]
    pub report: Option<PathBuf>,
}

/// Arguments for `matrix` subcommand.
#[derive(Parser)]
pub struct MatrixArgs {
    /// Matrix slice name (see `experiments::subset_experiments`).
    #[arg(long)]
    pub slice: String,
    /// Storage adapter slug.
    #[arg(long, default_value = "mem")]
    pub storage: String,
    /// Deployment shape slug.
    #[arg(long, default_value = "embedded")]
    pub deployment: String,
    /// Telemetry adapter slug.
    #[arg(long, default_value = "off")]
    pub telemetry: String,
    /// Topology slug.
    #[arg(long, default_value = "isolated-lab")]
    pub topology: String,
    /// Hardware profile slug written into reports.
    #[arg(long)]
    pub hardware: Option<String>,
    /// Warmup ticks before measured samples.
    #[arg(long, default_value_t = 0)]
    pub warmup: usize,
    /// Directory for JSON reports.
    #[arg(long, default_value = "profiling/chronon-bench/reports")]
    pub reports_dir: PathBuf,
}

/// Arguments for `scaling-curve` subcommand.
#[derive(Parser)]
pub struct ScalingCurveArgs {
    /// Curve kind: scaling curve or aggregate projection kind.
    pub kind: String,
    /// Storage adapter slug to filter reports.
    #[arg(long, default_value = "mem")]
    pub storage: String,
    /// Hardware profile slug to filter reports.
    #[arg(long)]
    pub hardware: Option<String>,
    /// Directory containing benchmark report JSONs.
    #[arg(long, default_value = "profiling/chronon-bench/reports")]
    pub reports_dir: PathBuf,
    /// Optional output path for the curve JSON.
    #[arg(long)]
    pub out: Option<PathBuf>,
}

/// Dispatch parsed CLI to the matching handler.
pub async fn dispatch(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Experiments => {
            for id in ALL_EXPERIMENT_IDS {
                println!("{id}");
            }
            Ok(())
        }
        Command::Run(args) => run_cmd::run_single(args).await,
        Command::Matrix(args) => matrix_cmd::run_matrix_subset(args).await,
        Command::ScalingCurve(args) => scaling_curve_cmd::dispatch(args),
        Command::Aggregate(args) => aggregate_cmd::dispatch(args),
    }
}

/// Resolve hardware profile from flag or env.
pub fn resolve_hardware(flag: Option<String>) -> String {
    flag.or_else(|| std::env::var("CHRONON_BENCH_HARDWARE").ok())
        .unwrap_or_else(|| "local".into())
}
