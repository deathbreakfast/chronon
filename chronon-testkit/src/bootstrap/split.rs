//! In-process coordinator–worker split bootstrap for matrix rows.

use std::sync::Arc;

use anyhow::Result;
use chronon_core::store::SchedulerStore;
use chronon_core::JsonScriptContextFactory;
use chronon_runtime::ChrononBuilder;

use super::{fresh_registry, telemetry_for_matrix, SplitHandle, BootstrapSession};

impl BootstrapSession {
    /// Start in-process coordinator and one worker task for split deployment rows.
    pub async fn spawn_coordinator_worker(&mut self) -> Result<()> {
        self.spawn_coordinator_worker_n(1).await
    }

    /// Start in-process coordinator and `worker_count` workers on the shared store.
    pub async fn spawn_coordinator_worker_n(&mut self, worker_count: u32) -> Result<()> {
        if self.split.is_some() {
            return Ok(());
        }
        self.spawn_split_runtime(worker_count, true).await
    }

    /// Start `worker_count` workers only (coordinator tick already driven via [`BootstrapSession::tick_once`]).
    pub async fn spawn_workers_n(&mut self, worker_count: u32) -> Result<()> {
        if self.split.is_some() {
            return Ok(());
        }
        self.spawn_split_runtime(worker_count, false).await
    }

    async fn spawn_split_runtime(&mut self, worker_count: u32, with_coordinator: bool) -> Result<()> {
        let count = worker_count.max(1);
        let store = self
            .store
            .clone()
            .ok_or_else(|| anyhow::anyhow!("install first"))?;
        let telemetry = telemetry_for_matrix(&self.matrix, &self.telemetry);
        let registry = fresh_registry();

        let store_dyn: Arc<dyn SchedulerStore> = store;

        let (coord_stop, coordinator_task) = if with_coordinator {
            let mut coordinator = ChrononBuilder::new()
                .scheduler_store(Arc::clone(&store_dyn))
                .context_factory(Arc::new(JsonScriptContextFactory))
                .telemetry_sink(Arc::clone(&telemetry))
                .script_registry(Arc::clone(&registry))
                .instance_id("coordinator-0")
                .coordinator_only()
                .build()
                .map_err(|e| anyhow::anyhow!("build coordinator: {e}"))?;
            coordinator.scheduler.init_partitions().await;
            let coord_stop = coordinator.shutdown_handle();
            let coordinator_task = tokio::spawn(async move { coordinator.run().await });
            (coord_stop, coordinator_task)
        } else {
            (
                Arc::new(tokio::sync::Notify::new()),
                tokio::spawn(async { Ok(()) }),
            )
        };

        let mut worker_stops = Vec::with_capacity(count as usize);
        let mut worker_tasks = Vec::with_capacity(count as usize);
        for i in 0..count {
            let worker_id = format!("worker-{i}");
            let mut worker = ChrononBuilder::new()
                .scheduler_store(Arc::clone(&store_dyn))
                .context_factory(Arc::new(JsonScriptContextFactory))
                .telemetry_sink(Arc::clone(&telemetry))
                .script_registry(Arc::clone(&registry))
                .instance_id(worker_id.clone())
                .worker("general")
                .build()
                .map_err(|e| anyhow::anyhow!("build worker {worker_id}: {e}"))?;
            let worker_stop = worker.shutdown_handle();
            worker_stops.push(worker_stop);
            worker_tasks.push(tokio::spawn(async move { worker.run().await }));
        }

        self.split = Some(SplitHandle {
            coord_stop,
            worker_stops,
            coordinator_task,
            worker_tasks,
        });
        Ok(())
    }
}
