//! Scaling-curve projection from collected report JSONs.

mod ch1_jobs;
mod ch7_aggregate;
mod ch7_data;
mod ch7_multibench;
mod ch7_pools;
mod ch7_workers;
mod ch7d_fleet;
mod chl_sustain;
mod common;

use std::path::{Path, PathBuf};

use anyhow::{bail, Result};

pub use ch1_jobs::ch1_job_curve;
pub use ch7_aggregate::ch7_aggregate;
pub use ch7_data::ch7_data_curve;
pub use ch7_multibench::ch7_multibench_curve;
pub use ch7_pools::ch7_pool_curve;
pub use ch7_workers::ch7_worker_curve;
pub use ch7d_fleet::ch7d_fleet_curve;
pub use chl_sustain::chl_sustain_curve;

/// Supported scaling-curve kinds.
pub const CURVE_KINDS: &[&str] = &[
    "ch7-worker-curve",
    "ch7-pool-curve",
    "ch7-data-curve",
    "ch7-multibench-curve",
    "ch7d-fleet-curve",
    "ch1-job-curve",
    "chl-sustain-curve",
];

/// Dispatch scaling-curve projection by kind name.
pub fn project_curve(
    kind: &str,
    storage: &str,
    hardware: &str,
    reports_dir: &Path,
    out: Option<PathBuf>,
) -> Result<()> {
    match kind {
        "ch7-worker-curve" => ch7_worker_curve(storage, hardware, reports_dir, out),
        "ch7-pool-curve" => ch7_pool_curve(storage, hardware, reports_dir, out),
        "ch7-data-curve" => ch7_data_curve(storage, hardware, reports_dir, out),
        "ch7-multibench-curve" => ch7_multibench_curve(storage, hardware, reports_dir, out),
        "ch7d-fleet-curve" => ch7d_fleet_curve(storage, hardware, reports_dir, out),
        "ch1-job-curve" => ch1_job_curve(storage, hardware, reports_dir, out),
        "chl-sustain-curve" => chl_sustain_curve(storage, hardware, reports_dir, out),
        other => bail!(
            "unknown curve kind {other}; use {}",
            CURVE_KINDS.join("|")
        ),
    }
}
