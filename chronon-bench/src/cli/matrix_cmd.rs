//! Matrix subset batch runs.

use anyhow::Result;

use crate::cli::bench_config::BenchConfigOverrides;
use crate::cli::run_cmd::{run_batch, BatchRunParams};
use crate::cli::{resolve_hardware, MatrixArgs};
use crate::experiments::subset_experiments;
use crate::report::BenchReport;

/// Run every experiment in a named matrix slice.
pub async fn run_matrix_subset(args: MatrixArgs) -> Result<()> {
    let hardware = resolve_hardware(args.hardware);
    let ids = subset_experiments(&args.slice)?;
    std::fs::create_dir_all(&args.reports_dir)?;

    for id in ids {
        let matrix_slug = format!(
            "{}-{}-{}-{}",
            args.storage, args.deployment, args.topology, args.telemetry
        );
        let path = args.reports_dir.join(BenchReport::report_filename(
            id,
            &matrix_slug,
            &hardware,
        ));
        println!("running {id} …");
        run_batch(BatchRunParams {
            experiment: id,
            storage: &args.storage,
            deployment: &args.deployment,
            telemetry: &args.telemetry,
            topology: &args.topology,
            hardware: &hardware,
            warmup: args.warmup,
            overrides: BenchConfigOverrides::default(),
            report_path: Some(&path),
        })
        .await?;
    }
    Ok(())
}
