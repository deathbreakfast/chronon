//! Matrix-driven bootstrap for e2e and bench.

mod embedded;
mod env_guard;
mod multibench_store;
mod session;
mod split;

use std::sync::Arc;

use chronon_core::store::SchedulerStore;
use chronon_telemetry::RecordingSink;
use tokio::sync::Notify;
use tokio::task::JoinHandle;

use crate::fixtures::register_builtin_probes;
use crate::matrix::{MatrixSpec, TelemetryAdapter};
use chronon_executor::ScriptRegistry;

pub(super) fn fresh_registry() -> Arc<ScriptRegistry> {
    let mut registry = ScriptRegistry::new();
    register_builtin_probes(&mut registry);
    Arc::new(registry)
}

pub(super) fn telemetry_for_matrix(
    matrix: &MatrixSpec,
    recording: &Arc<RecordingSink>,
) -> Arc<dyn chronon_telemetry::TelemetrySink> {
    match matrix.telemetry {
        TelemetryAdapter::Off => Arc::clone(recording) as Arc<dyn chronon_telemetry::TelemetrySink>,
        TelemetryAdapter::Console => {
            Arc::new(chronon_telemetry::ConsoleSink) as Arc<dyn chronon_telemetry::TelemetrySink>
        }
    }
}

/// Background embedded runtime started by [`BootstrapSession::spawn_embedded`].
pub struct EmbeddedHandle {
    pub(super) stop: Arc<Notify>,
    pub(super) task: JoinHandle<chronon_core::Result<()>>,
}

impl EmbeddedHandle {
    /// Signal shutdown and await the embedded runtime task.
    pub async fn shutdown(self) {
        // `Notify::notify_waiters` does not store a permit. If the runtime task has not
        // reached `notified().await` yet (still in init), a single notify is lost and
        // `task.await` hangs forever — re-arm until the task exits.
        let stop = self.stop;
        let mut task = self.task;
        loop {
            stop.notify_waiters();
            tokio::select! {
                _ = &mut task => break,
                () = tokio::time::sleep(std::time::Duration::from_millis(25)) => {}
            }
        }
    }
}

/// In-process coordinator + worker tasks for split deployment matrix rows.
pub struct SplitHandle {
    pub(super) coord_stop: Arc<Notify>,
    pub(super) worker_stops: Vec<Arc<Notify>>,
    pub(super) coordinator_task: JoinHandle<chronon_core::Result<()>>,
    pub(super) worker_tasks: Vec<JoinHandle<chronon_core::Result<()>>>,
}

impl SplitHandle {
    /// Stop coordinator and worker tasks and await their completion.
    pub async fn shutdown(self) {
        // Pulse notifies until tasks exit (Notify::notify_waiters stores no permit).
        let coord_stop = self.coord_stop;
        let worker_stops = self.worker_stops;
        let mut coordinator_task = self.coordinator_task;
        let worker_tasks = self.worker_tasks;
        loop {
            coord_stop.notify_waiters();
            for stop in &worker_stops {
                stop.notify_waiters();
            }
            if coordinator_task.is_finished()
                && worker_tasks
                    .iter()
                    .all(tokio::task::JoinHandle::is_finished)
            {
                let _ = (&mut coordinator_task).await;
                for task in worker_tasks {
                    let _ = task.await;
                }
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
    }
}

/// Bootstraps a Chronon instance for one matrix row.
pub struct BootstrapSession {
    pub(super) matrix: MatrixSpec,
    pub(super) store: Option<Arc<dyn SchedulerStore>>,
    pub(super) telemetry: Arc<RecordingSink>,
    pub(super) chronon: Option<chronon_runtime::Chronon>,
    pub(super) embedded: Option<EmbeddedHandle>,
    pub(super) split: Option<SplitHandle>,
    pub(super) ready: bool,
    pub(super) num_partitions: u32,
    pub(super) env_guard: Option<env_guard::EnvGuard>,
    pub(super) sqlite_temp: Option<tempfile::TempDir>,
    pub(super) postgres_schema: Option<String>,
}

impl BootstrapSession {
    /// Create a session for one matrix row (call [`BootstrapSession::install`] before use).
    pub fn new(matrix: MatrixSpec) -> Self {
        Self {
            matrix,
            store: None,
            telemetry: Arc::new(RecordingSink::new()),
            chronon: None,
            embedded: None,
            split: None,
            ready: false,
            num_partitions: 4,
            env_guard: None,
            sqlite_temp: None,
            postgres_schema: None,
        }
    }

    /// Override partition count for this session (default 4, applied on [`BootstrapSession::install`]).
    pub fn with_num_partitions(mut self, n: u32) -> Self {
        self.num_partitions = n.max(1);
        self
    }

    /// Matrix row this session was constructed for.
    pub fn matrix(&self) -> &MatrixSpec {
        &self.matrix
    }

    /// Whether [`BootstrapSession::install`] completed successfully.
    pub fn is_ready(&self) -> bool {
        self.ready
    }

    /// Shared recording sink for telemetry assertions in correctness scenarios.
    pub fn telemetry(&self) -> Arc<RecordingSink> {
        Arc::clone(&self.telemetry)
    }

    /// Register a built-in script probe (noop today; probes install on Chronon build).
    pub fn register_probe(&mut self, _probe: crate::scenario::ScriptProbeKind) {
        // Built-in probes are registered on each Chronon build.
    }
}
