//! Script registry lookup, context build, and async run lifecycle.
//!
//! Resolves registered script handlers, builds execution context from stored actor JSON,
//! and dispatches async runs with lifecycle events back to the runtime.
//!
//! # Documentation map
//!
//! - **Register handlers** — [`ScriptRegistry`], link-time inventory via `#[chronon::script]`
//! - **Dispatch runs** — [`Executor::spawn_run`], [`execute_script`]
//! - **Observe lifecycle** — [`ExecutorEvent`]
//!
//! # Notes
//!
//! [`Executor::spawn_run`] uses run-level `params_json`, not job defaults. Missing scripts
//! surface as [`ChrononError::ScriptNotFound`](chronon_core::ChrononError::ScriptNotFound).

mod descriptor;
mod invoke;
mod registry;

pub use descriptor::{InvokeFn, ScriptDescriptor};
pub use invoke::{execute_script, ExecuteScriptRequest};
pub use registry::{ScriptDescriptorRef, ScriptRegistry};

use std::sync::Arc;

use chrono::Utc;
use chronon_core::{ContextFactory, Job, Run};
use chronon_telemetry::TelemetrySink;
use tokio::sync::mpsc;

/// Event sent from the executor to the runtime for run status updates.
///
/// Consumed by `chronon-runtime` to persist run state and forward metrics.
#[derive(Debug, Clone)]
pub enum ExecutorEvent {
    /// A run task was spawned and execution has begun.
    RunStarted {
        /// Run identifier matching [`Run::run_id`](chronon_core::Run::run_id).
        run_id: String,
    },
    /// Handler returned successfully.
    RunCompleted {
        /// Run identifier matching [`Run::run_id`](chronon_core::Run::run_id).
        run_id: String,
        /// Wall-clock duration from spawn to handler completion, in milliseconds.
        duration_ms: i64,
    },
    /// Handler returned an error or context build failed.
    RunFailed {
        /// Run identifier matching [`Run::run_id`](chronon_core::Run::run_id).
        run_id: String,
        /// Display-formatted error message for logs and persistence.
        error: String,
    },
}

/// Executor for running registered scripts against scheduled jobs.
///
/// Constructed by `ChrononBuilder` in `chronon-runtime` and called when workers claim runs.
pub struct Executor {
    /// Script catalog used to resolve handler functions by name.
    pub registry: Arc<ScriptRegistry>,
    /// Rebuilds [`ScriptContext`](chronon_core::ScriptContext) from job `actor_json`.
    pub context_factory: Arc<dyn ContextFactory>,
    /// Metrics and structured error events for invoke phases.
    pub telemetry: Arc<dyn TelemetrySink>,
    event_tx: mpsc::UnboundedSender<ExecutorEvent>,
}

impl Executor {
    /// Builds an executor wired to the given registry, factory, telemetry, and event channel.
    ///
    /// The runtime typically clones [`Self::event_sender`] before passing `event_tx` so both
    /// sides can send lifecycle updates.
    pub fn new(
        registry: Arc<ScriptRegistry>,
        context_factory: Arc<dyn ContextFactory>,
        telemetry: Arc<dyn TelemetrySink>,
        event_tx: mpsc::UnboundedSender<ExecutorEvent>,
    ) -> Self {
        Self {
            registry,
            context_factory,
            telemetry,
            event_tx,
        }
    }

    /// Clones the unbounded sender for [`ExecutorEvent`] lifecycle updates.
    ///
    /// Used by the runtime to subscribe without holding an [`Executor`] reference.
    pub fn event_sender(&self) -> mpsc::UnboundedSender<ExecutorEvent> {
        self.event_tx.clone()
    }

    /// Returns the number of scripts currently registered.
    pub fn script_count(&self) -> usize {
        self.registry.len()
    }

    /// Spawn asynchronous execution for one run of the given job.
    ///
    /// Emits [`ExecutorEvent::RunStarted`] immediately, then invokes the script via
    /// [`execute_script`]. Run `params_json` takes precedence over job defaults.
    pub fn spawn_run(&self, job: &Job, run: Run) {
        let registry = Arc::clone(&self.registry);
        let context_factory = Arc::clone(&self.context_factory);
        let telemetry = Arc::clone(&self.telemetry);
        let event_tx = self.event_tx.clone();

        let script_name = job.script_name.clone();
        let job_name = job.job_name.clone();
        let params_json = run.params_json.clone();
        let actor_json = job.actor_json.clone();
        let run_id = run.run_id;

        tokio::spawn(async move {
            let _ = event_tx.send(ExecutorEvent::RunStarted {
                run_id: run_id.clone(),
            });
            telemetry.record_counter(
                "chronon_runs_started",
                &[("script", script_name.as_str()), ("job", job_name.as_str())],
                1,
            );

            let started = Utc::now();
            let result = invoke::execute_script(invoke::ExecuteScriptRequest {
                registry: &registry,
                context_factory: &context_factory,
                telemetry: &telemetry,
                script_name: &script_name,
                actor_json: &actor_json,
                params_json,
                job_name: &job_name,
                run_id: &run_id,
            })
            .await;

            let duration_ms = (Utc::now() - started).num_milliseconds();
            match result {
                Ok(()) => {
                    let _ = event_tx.send(ExecutorEvent::RunCompleted {
                        run_id: run_id.clone(),
                        duration_ms,
                    });
                    telemetry.record_counter(
                        "chronon_runs_completed",
                        &[("script", script_name.as_str()), ("job", job_name.as_str())],
                        1,
                    );
                }
                Err(e) => {
                    let error_msg = e.to_string();
                    let _ = event_tx.send(ExecutorEvent::RunFailed {
                        run_id: run_id.clone(),
                        error: error_msg.clone(),
                    });
                    telemetry.record_counter(
                        "chronon_runs_failed",
                        &[("script", script_name.as_str()), ("job", job_name.as_str())],
                        1,
                    );
                    telemetry.log_event(
                        "chronon_run_failed",
                        &[
                            ("run_id", run_id.as_str()),
                            ("job", job_name.as_str()),
                            ("error", error_msg.as_str()),
                        ],
                    );
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chronon_core::{NoOpContextFactory, Result, ScriptContext};
    use serde_json::{json, Value};
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::Mutex;

    static LAST_PARAMS: Mutex<Option<Value>> = Mutex::new(None);

    fn param_probe(
        _ctx: Box<dyn ScriptContext>,
        params: Value,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send>> {
        Box::pin(async move {
            *LAST_PARAMS.lock().unwrap() = Some(params);
            Ok(())
        })
    }

    #[tokio::test]
    async fn spawn_run_uses_run_params() {
        *LAST_PARAMS.lock().unwrap() = None;
        let registry = Arc::new({
            let mut r = ScriptRegistry::new();
            r.register(ScriptDescriptor::new("probe", param_probe));
            r
        });
        let (tx, mut rx) = mpsc::unbounded_channel();
        let executor = Executor::new(
            registry,
            Arc::new(NoOpContextFactory),
            Arc::new(chronon_telemetry::NoOpSink),
            tx,
        );

        let mut job = Job::new("job", "probe");
        let mut run = chronon_core::Run::for_job(&job.job_id, "probe", Utc::now());
        run.params_json = json!({ "source": "run" });
        job.params_json = json!({ "source": "job" });

        executor.spawn_run(&job, run);

        for _ in 0..20 {
            if let Some(ExecutorEvent::RunCompleted { .. }) = rx.recv().await {
                break;
            }
        }
        assert_eq!(
            *LAST_PARAMS.lock().unwrap(),
            Some(json!({ "source": "run" }))
        );
    }
}
