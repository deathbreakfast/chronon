//! Matrix dimension parsing for CLI flags.

use anyhow::Result;
use chronon_testkit::{DeploymentKind, MatrixSpec, StorageAdapter, TelemetryAdapter, Topology};

/// Build a [`MatrixSpec`] from CLI flag strings (kebab-case slugs).
pub fn matrix_from_cli(
    storage: &str,
    deployment: &str,
    telemetry: &str,
    topology: &str,
) -> Result<MatrixSpec> {
    Ok(MatrixSpec {
        storage: StorageAdapter::parse(storage)?,
        deployment: DeploymentKind::parse(deployment)?,
        topology: Topology::parse(topology)?,
        telemetry: TelemetryAdapter::parse(telemetry)?,
    })
}
