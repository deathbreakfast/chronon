//! Scaling-curve projection CLI handler.

use anyhow::Result;

use crate::cli::{resolve_hardware, ScalingCurveArgs};
use crate::projection::project_curve;

/// Dispatch scaling-curve subcommand.
pub fn dispatch(args: ScalingCurveArgs) -> Result<()> {
    let hardware = resolve_hardware(args.hardware);
    project_curve(
        &args.kind,
        &args.storage,
        &hardware,
        &args.reports_dir,
        args.out,
    )
}
