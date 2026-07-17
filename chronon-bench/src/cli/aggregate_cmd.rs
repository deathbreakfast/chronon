//! BM-CH7 multibench report aggregation CLI handler.

use anyhow::Result;

use crate::cli::{resolve_hardware, AggregateArgs};
use crate::projection;

/// Dispatch aggregate subcommand.
pub fn dispatch(args: AggregateArgs) -> Result<()> {
    let hardware = resolve_hardware(args.hardware);
    projection::ch7_aggregate(
        &args.storage,
        &hardware,
        &args.reports_dir,
        &args.cell_prefix,
    )
}
