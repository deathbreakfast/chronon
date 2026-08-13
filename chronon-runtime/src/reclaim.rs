//! Background reclaim of expired worker run leases.

use std::sync::Arc;
use std::time::Duration;

use chronon_core::store::SchedulerStore;
use tokio::sync::Notify;
use tokio::time::sleep;
use tracing::{info, warn};

const RECLAIM_INTERVAL_SECS: u64 = 15;

/// Periodically reset `claimed`/`running` runs whose leases have expired back to `queued`.
pub async fn run_reclaim_loop(store: Arc<dyn SchedulerStore>, shutdown: Arc<Notify>) {
    loop {
        tokio::select! {
            () = shutdown.notified() => break,
            () = sleep(Duration::from_secs(RECLAIM_INTERVAL_SECS)) => {
                let now = chrono::Utc::now();
                match store.reclaim_expired_run_leases(now).await {
                    Ok(ids) if !ids.is_empty() => {
                        info!(count = ids.len(), "reclaimed expired chronon run leases");
                    }
                    Ok(_) => {}
                    Err(e) => {
                        warn!(error = %e, "failed to reclaim expired chronon run leases");
                    }
                }
            }
        }
    }
}
