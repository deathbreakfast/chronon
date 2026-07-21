//! High-level scheduler API for embedded and coordinator hosts.

use std::sync::Arc;

use chrono::Utc;
use chronon_core::store::SchedulerStore;
use chronon_telemetry::TelemetrySink;

use crate::partition_assigner::PartitionAssigner;
use crate::tick_loop::{run_one_tick, TickResult};
use crate::SchedulerConfig;

/// Runs scheduler ticks against a [`SchedulerStore`].
///
/// Construct via [`Self::new`], call [`Self::init_partitions`] once at boot, then
/// [`Self::tick_once`] or delegate to [`crate::run_coordinator_tick_loop`].
pub struct Scheduler {
    config: SchedulerConfig,
    store: Arc<dyn SchedulerStore>,
    telemetry: Arc<dyn TelemetrySink>,
    assigner: Arc<PartitionAssigner>,
}

impl Scheduler {
    /// Builds a scheduler wired to the given config, store, and telemetry sink.
    ///
    /// Creates an internal [`PartitionAssigner`] using `config.instance_id` and
    /// `config.num_partitions`.
    pub fn new(
        config: SchedulerConfig,
        store: Arc<dyn SchedulerStore>,
        telemetry: Arc<dyn TelemetrySink>,
    ) -> Self {
        let assigner = Arc::new(PartitionAssigner::new(
            store.clone(),
            telemetry.clone(),
            config.instance_id.clone(),
            config.num_partitions,
        ));
        Self {
            config,
            store,
            telemetry,
            assigner,
        }
    }

    /// Returns the partition assigner shared with coordinator tick loops.
    pub fn assigner(&self) -> Arc<PartitionAssigner> {
        Arc::clone(&self.assigner)
    }

    /// Returns this coordinator's unique instance identifier.
    pub fn instance_id(&self) -> &str {
        &self.config.instance_id
    }

    /// Clones the backing scheduler store handle.
    pub fn store(&self) -> Arc<dyn SchedulerStore> {
        Arc::clone(&self.store)
    }

    /// Clones the telemetry sink used for tick metrics and events.
    pub fn telemetry(&self) -> Arc<dyn TelemetrySink> {
        Arc::clone(&self.telemetry)
    }

    /// Initialize partition ownership (embedded assigns all partitions locally).
    pub async fn init_partitions(&self) {
        if self.config.embedded {
            self.assigner.assign_all_embedded().await;
        } else if self.assigner.refresh_leases().await.is_err() {
            self.telemetry.log_event(
                "chronon_scheduler_warn",
                &[
                    ("component", "scheduler"),
                    ("message", "initial partition refresh failed"),
                ],
            );
        }
    }

    /// Execute one scheduler tick (due query + enqueue + telemetry).
    #[tracing::instrument(skip(self), fields(instance_id = %self.config.instance_id))]
    pub async fn tick_once(&self) -> chronon_core::Result<TickResult> {
        let mut draining = false;
        Ok(run_one_tick(
            &self.store,
            &self.telemetry,
            &self.config.instance_id,
            &self.assigner,
            &mut draining,
        )
        .await)
    }

    /// Simple due-job count for diagnostics (legacy API).
    ///
    /// Queries up to 10,000 due job ids across owned partitions; not used on the hot path.
    pub async fn due_job_count(&self) -> chronon_core::Result<usize> {
        let owned = self.assigner.owned_partitions().await;
        let ids = self
            .store
            .find_due_job_ids_in_partitions(&owned, Utc::now(), 10_000)
            .await?;
        Ok(ids.len())
    }

    /// Returns the configured tick interval in milliseconds.
    pub fn tick_interval_ms(&self) -> u64 {
        self.config.tick_interval_ms
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chronon_backend_mem::InMemorySchedulerStore;
    use chronon_core::{Job, ScheduleKind};
    use chronon_telemetry::NoOpSink;

    #[tokio::test]
    async fn tick_enqueues_due_job() {
        let store = Arc::new(InMemorySchedulerStore::new());
        let mut job = Job::new("test-job", "test_script");
        job.schedule_kind = ScheduleKind::Cron;
        job.cron_expr = Some("* * * * *".to_string());
        job.next_run_at = Some(Utc::now() - chrono::Duration::seconds(1));
        job.partition_hash = Some(0);
        job.enabled = true;
        store.upsert_job(&job).await.unwrap();

        let scheduler = Scheduler::new(
            SchedulerConfig {
                embedded: true,
                num_partitions: 1,
                ..Default::default()
            },
            store.clone(),
            Arc::new(NoOpSink),
        );
        scheduler.init_partitions().await;

        let result = scheduler.tick_once().await.unwrap();
        assert!(result.due_count >= 1 || result.enqueued >= 1);
    }
}
