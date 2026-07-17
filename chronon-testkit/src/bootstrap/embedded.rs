//! Embedded deployment loop spawning for matrix bootstrap.

use anyhow::Result;

use crate::matrix::DeploymentKind;

use super::{EmbeddedHandle, BootstrapSession};

impl BootstrapSession {
    /// Start background loops appropriate for the matrix deployment shape.
    pub async fn spawn_embedded(&mut self) -> Result<()> {
        match self.matrix.deployment {
            DeploymentKind::Embedded | DeploymentKind::RemoteClient => {
                self.spawn_embedded_loops().await
            }
            DeploymentKind::CoordinatorWorker => self.spawn_coordinator_worker().await,
        }
    }

    pub(super) async fn spawn_embedded_loops(&mut self) -> Result<()> {
        if self.embedded.is_some() {
            return Ok(());
        }
        if self.chronon.is_none() {
            self.build_chronon()?;
        }
        let mut chronon = self
            .chronon
            .take()
            .ok_or_else(|| anyhow::anyhow!("build chronon before spawn_embedded"))?;
        chronon.scheduler.init_partitions().await;
        let stop = chronon.shutdown_handle();
        let task = tokio::spawn(async move { chronon.run().await });
        self.embedded = Some(EmbeddedHandle { stop, task });
        Ok(())
    }
}
