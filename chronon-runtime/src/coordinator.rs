//! Coordinator-only deployment: tick loop without workers.

use std::sync::Arc;

use chronon_scheduler::{run_coordinator_tick_loop, Scheduler};
use chronon_telemetry::TelemetrySink;
use tokio::sync::Notify;

/// Run partition assigner and coordinator tick loop until `shutdown` is notified.
///
/// Called from [`Chronon::run`](crate::Chronon::run) for [`DeploymentShape::CoordinatorOnly`](crate::DeploymentShape::CoordinatorOnly).
pub async fn run_coordinator_loops(
    scheduler: Arc<Scheduler>,
    telemetry: Arc<dyn TelemetrySink>,
    shutdown: Arc<Notify>,
) {
    scheduler.init_partitions().await;

    let assigner = scheduler.assigner();
    let partition_shutdown = Arc::clone(&shutdown);
    let partition_assigner = Arc::clone(&assigner);
    tokio::spawn(async move {
        partition_assigner.run_lease_loop(partition_shutdown).await;
    });

    run_coordinator_tick_loop(
        scheduler.store(),
        telemetry,
        scheduler.instance_id().to_string(),
        assigner,
        shutdown,
    )
    .await;
}
