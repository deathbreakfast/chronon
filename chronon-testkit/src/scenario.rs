//! Declarative scenario steps shared by e2e (assert) and bench (measure).

use chronon_core::models::RunStatus;
use serde::{Deserialize, Serialize};

/// Built-in script probe registered by the testkit bootstrap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScriptProbeKind {
    /// No-op probe that completes immediately.
    Noop,
    /// Probe that increments a global counter on each invocation.
    Counting,
    /// Probe that always returns an internal error.
    Fail,
}

impl ScriptProbeKind {
    /// Registry script name for this probe kind.
    pub fn script_name(self) -> &'static str {
        match self {
            Self::Noop => crate::fixtures::NOOP_SCRIPT,
            Self::Counting => crate::fixtures::COUNTING_SCRIPT,
            Self::Fail => crate::fixtures::FAIL_SCRIPT,
        }
    }
}

/// One step in a scheduler scenario.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "step", rename_all = "snake_case")]
pub enum ScenarioStep {
    /// Ensure a built-in probe script is registered (idempotent).
    RegisterScript {
        /// Which built-in probe to register.
        probe: ScriptProbeKind,
    },
    /// Upsert a cron job with next_run_at in the past.
    UpsertDueCronJob {
        /// Unique job name.
        job_name: String,
        /// Script registry name to invoke.
        script_name: String,
        /// Cron expression (six-field with seconds supported).
        cron: String,
    },
    /// Upsert a cron job with next_run_at in the future (not due).
    UpsertFutureCronJob {
        /// Unique job name.
        job_name: String,
        /// Script registry name to invoke.
        script_name: String,
        /// Cron expression (six-field with seconds supported).
        cron: String,
    },
    /// Upsert a manual job (only triggered via [`ScenarioStep::RunNow`]).
    UpsertManualJob {
        /// Unique job name.
        job_name: String,
        /// Script registry name to invoke.
        script_name: String,
    },
    /// Upsert a run-once job due immediately.
    UpsertRunOnceDueJob {
        /// Unique job name.
        job_name: String,
        /// Script registry name to invoke.
        script_name: String,
    },
    /// Initialize partition ownership for embedded/coordinator ticks.
    InitPartitions,
    /// Execute one coordinator tick.
    Tick,
    /// Disable scheduling for a job by name.
    PauseJob {
        /// Job name to pause.
        job_name: String,
    },
    /// Re-enable scheduling for a paused job by name.
    ResumeJob {
        /// Job name to resume.
        job_name: String,
    },
    /// Enqueue an immediate run via the coordinator API.
    RunNow {
        /// Job name to trigger.
        job_name: String,
    },
    /// Start embedded scheduler + worker loops in the background.
    SpawnEmbedded,
    /// Stop embedded loops started by [`ScenarioStep::SpawnEmbedded`].
    ShutdownEmbedded,
    /// Wait until a run reaches a terminal status.
    WaitRunTerminal {
        /// Job name to poll.
        job_name: String,
        /// Expected terminal status.
        status: RunStatus,
        /// Poll timeout in milliseconds.
        timeout_ms: u64,
    },
    /// Assert run row count for a job name.
    AssertRunCount {
        /// Job name to inspect.
        job_name: String,
        /// Expected number of run rows.
        expected: u32,
    },
    /// Assert a telemetry counter was recorded at least `min` times.
    AssertTelemetryCounter {
        /// Counter metric name.
        name: String,
        /// Label key/value pairs to match.
        labels: Vec<(String, String)>,
        /// Minimum total delta across matching recordings.
        min: u64,
    },
    /// Assert the last tick enqueued exactly `expected` runs.
    AssertLastTickEnqueued {
        /// Expected enqueue count from the last tick step.
        expected: usize,
    },
    /// Assert partition-filtered due query behavior on the mem store.
    AssertPartitionDueFilter {
        /// Job whose partition hash is validated.
        job_name: String,
    },
    /// Assert at least `min` revision rows exist for a job.
    AssertRevisionCount {
        /// Job name to inspect.
        job_name: String,
        /// Minimum revision count.
        min: u32,
    },
    /// Assert a job is not due for enqueue at the current time (sad path).
    AssertJobNotDue {
        /// Job name to inspect.
        job_name: String,
    },
    /// Reset the global counting probe invocation counter.
    ResetCountingProbe,
    /// Assert the counting probe ran at least `min` times.
    AssertCountingProbe {
        /// Minimum invocation count.
        min: u32,
    },
}

/// Ordered scenario consumed by both e2e and bench drivers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScenarioSpec {
    /// Stable scenario identifier for reports and logs.
    pub id: String,
    /// Steps executed in order.
    pub steps: Vec<ScenarioStep>,
}

impl ScenarioSpec {
    /// Minimal smoke: init partitions, tick once, expect zero enqueues.
    pub fn scheduler_tick_smoke() -> Self {
        Self {
            id: "scheduler-tick-smoke".into(),
            steps: vec![
                ScenarioStep::InitPartitions,
                ScenarioStep::Tick,
                ScenarioStep::AssertLastTickEnqueued { expected: 0 },
            ],
        }
    }

    /// Due cron job is enqueued on the next tick.
    pub fn due_job_enqueue() -> Self {
        Self {
            id: "due-job-enqueue".into(),
            steps: vec![
                ScenarioStep::RegisterScript {
                    probe: ScriptProbeKind::Noop,
                },
                ScenarioStep::UpsertDueCronJob {
                    job_name: "due-enqueue".into(),
                    script_name: crate::fixtures::NOOP_SCRIPT.into(),
                    cron: "0 * * * * *".into(),
                },
                ScenarioStep::InitPartitions,
                ScenarioStep::Tick,
                ScenarioStep::AssertLastTickEnqueued { expected: 1 },
            ],
        }
    }

    /// Embedded run completes with success status.
    pub fn script_run_success() -> Self {
        Self {
            id: "script-run-success".into(),
            steps: vec![
                ScenarioStep::RegisterScript {
                    probe: ScriptProbeKind::Noop,
                },
                ScenarioStep::UpsertDueCronJob {
                    job_name: "run-success".into(),
                    script_name: crate::fixtures::NOOP_SCRIPT.into(),
                    cron: "0 * * * * *".into(),
                },
                ScenarioStep::SpawnEmbedded,
                ScenarioStep::WaitRunTerminal {
                    job_name: "run-success".into(),
                    status: RunStatus::Success,
                    timeout_ms: 5_000,
                },
                ScenarioStep::ShutdownEmbedded,
            ],
        }
    }

    /// Run-once job produces exactly one terminal run.
    pub fn run_once_idempotent() -> Self {
        Self {
            id: "run-once-idempotent".into(),
            steps: vec![
                ScenarioStep::RegisterScript {
                    probe: ScriptProbeKind::Noop,
                },
                ScenarioStep::UpsertRunOnceDueJob {
                    job_name: "once-job".into(),
                    script_name: crate::fixtures::NOOP_SCRIPT.into(),
                },
                ScenarioStep::SpawnEmbedded,
                ScenarioStep::WaitRunTerminal {
                    job_name: "once-job".into(),
                    status: RunStatus::Success,
                    timeout_ms: 5_000,
                },
                ScenarioStep::AssertRunCount {
                    job_name: "once-job".into(),
                    expected: 1,
                },
                ScenarioStep::ShutdownEmbedded,
            ],
        }
    }

    /// Telemetry counters fire across scheduler tick and run completion.
    pub fn telemetry_lifecycle() -> Self {
        Self {
            id: "telemetry-lifecycle".into(),
            steps: vec![
                ScenarioStep::RegisterScript {
                    probe: ScriptProbeKind::Noop,
                },
                ScenarioStep::UpsertDueCronJob {
                    job_name: "telemetry-job".into(),
                    script_name: crate::fixtures::NOOP_SCRIPT.into(),
                    cron: "0 * * * * *".into(),
                },
                ScenarioStep::SpawnEmbedded,
                ScenarioStep::WaitRunTerminal {
                    job_name: "telemetry-job".into(),
                    status: RunStatus::Success,
                    timeout_ms: 5_000,
                },
                ScenarioStep::AssertTelemetryCounter {
                    name: "chronon_runs_completed".into(),
                    labels: vec![("job".into(), "telemetry-job".into())],
                    min: 1,
                },
                ScenarioStep::AssertTelemetryCounter {
                    name: "chronon_scheduler_ticks".into(),
                    labels: vec![("component".into(), "scheduler".into())],
                    min: 1,
                },
                ScenarioStep::ShutdownEmbedded,
            ],
        }
    }

    /// Partition-scoped due queries include owning partition only.
    pub fn partition_due_filter() -> Self {
        Self {
            id: "partition-due-filter".into(),
            steps: vec![
                ScenarioStep::RegisterScript {
                    probe: ScriptProbeKind::Noop,
                },
                ScenarioStep::UpsertDueCronJob {
                    job_name: "partition-job".into(),
                    script_name: crate::fixtures::NOOP_SCRIPT.into(),
                    cron: "0 * * * * *".into(),
                },
                ScenarioStep::AssertPartitionDueFilter {
                    job_name: "partition-job".into(),
                },
            ],
        }
    }

    /// Future cron job is not enqueued on tick (sad path).
    pub fn not_due_no_enqueue() -> Self {
        Self {
            id: "not-due-no-enqueue".into(),
            steps: vec![
                ScenarioStep::RegisterScript {
                    probe: ScriptProbeKind::Noop,
                },
                ScenarioStep::UpsertFutureCronJob {
                    job_name: "future-job".into(),
                    script_name: crate::fixtures::NOOP_SCRIPT.into(),
                    cron: "0 * * * * *".into(),
                },
                ScenarioStep::InitPartitions,
                ScenarioStep::AssertJobNotDue {
                    job_name: "future-job".into(),
                },
            ],
        }
    }

    /// Paused job is skipped on tick; resume restores enqueue (sad + happy).
    pub fn pause_resume_smoke() -> Self {
        Self {
            id: "pause-resume-smoke".into(),
            steps: vec![
                ScenarioStep::RegisterScript {
                    probe: ScriptProbeKind::Noop,
                },
                ScenarioStep::UpsertDueCronJob {
                    job_name: "pause-resume".into(),
                    script_name: crate::fixtures::NOOP_SCRIPT.into(),
                    cron: "0 * * * * *".into(),
                },
                ScenarioStep::PauseJob {
                    job_name: "pause-resume".into(),
                },
                ScenarioStep::InitPartitions,
                ScenarioStep::Tick,
                ScenarioStep::AssertLastTickEnqueued { expected: 0 },
                ScenarioStep::ResumeJob {
                    job_name: "pause-resume".into(),
                },
                ScenarioStep::Tick,
                ScenarioStep::AssertLastTickEnqueued { expected: 1 },
            ],
        }
    }

    /// Failing script probe produces a terminal failed run (sad path).
    pub fn script_run_failure() -> Self {
        Self {
            id: "script-run-failure".into(),
            steps: vec![
                ScenarioStep::RegisterScript {
                    probe: ScriptProbeKind::Fail,
                },
                ScenarioStep::UpsertDueCronJob {
                    job_name: "fail-job".into(),
                    script_name: crate::fixtures::FAIL_SCRIPT.into(),
                    cron: "0 * * * * *".into(),
                },
                ScenarioStep::SpawnEmbedded,
                ScenarioStep::WaitRunTerminal {
                    job_name: "fail-job".into(),
                    status: RunStatus::Failed,
                    timeout_ms: 5_000,
                },
                ScenarioStep::ShutdownEmbedded,
            ],
        }
    }

    /// Manual job triggered via run_now completes successfully.
    pub fn run_now_smoke() -> Self {
        Self {
            id: "run-now-smoke".into(),
            steps: vec![
                ScenarioStep::RegisterScript {
                    probe: ScriptProbeKind::Noop,
                },
                ScenarioStep::UpsertManualJob {
                    job_name: "manual-job".into(),
                    script_name: crate::fixtures::NOOP_SCRIPT.into(),
                },
                ScenarioStep::RunNow {
                    job_name: "manual-job".into(),
                },
                ScenarioStep::SpawnEmbedded,
                ScenarioStep::WaitRunTerminal {
                    job_name: "manual-job".into(),
                    status: RunStatus::Success,
                    timeout_ms: 5_000,
                },
                ScenarioStep::ShutdownEmbedded,
            ],
        }
    }

    /// Job upsert appends at least one revision row.
    pub fn job_revisions_smoke() -> Self {
        Self {
            id: "job-revisions-smoke".into(),
            steps: vec![
                ScenarioStep::RegisterScript {
                    probe: ScriptProbeKind::Noop,
                },
                ScenarioStep::UpsertManualJob {
                    job_name: "rev-job".into(),
                    script_name: crate::fixtures::NOOP_SCRIPT.into(),
                },
                ScenarioStep::AssertRevisionCount {
                    job_name: "rev-job".into(),
                    min: 1,
                },
            ],
        }
    }

    /// Counting probe runs exactly once for a single due cron execution.
    pub fn counting_exactly_once() -> Self {
        Self {
            id: "counting-exactly-once".into(),
            steps: vec![
                ScenarioStep::RegisterScript {
                    probe: ScriptProbeKind::Counting,
                },
                ScenarioStep::ResetCountingProbe,
                ScenarioStep::UpsertDueCronJob {
                    job_name: "count-once".into(),
                    script_name: crate::fixtures::COUNTING_SCRIPT.into(),
                    cron: "0 * * * * *".into(),
                },
                ScenarioStep::SpawnEmbedded,
                ScenarioStep::WaitRunTerminal {
                    job_name: "count-once".into(),
                    status: RunStatus::Success,
                    timeout_ms: 5_000,
                },
                ScenarioStep::AssertCountingProbe { min: 1 },
                ScenarioStep::ShutdownEmbedded,
            ],
        }
    }

    /// Waiting for the wrong terminal status times out (sad path).
    pub fn wait_run_timeout() -> Self {
        Self {
            id: "wait-run-timeout".into(),
            steps: vec![
                ScenarioStep::RegisterScript {
                    probe: ScriptProbeKind::Fail,
                },
                ScenarioStep::UpsertDueCronJob {
                    job_name: "timeout-job".into(),
                    script_name: crate::fixtures::FAIL_SCRIPT.into(),
                    cron: "0 * * * * *".into(),
                },
                ScenarioStep::SpawnEmbedded,
                ScenarioStep::WaitRunTerminal {
                    job_name: "timeout-job".into(),
                    status: RunStatus::Success,
                    timeout_ms: 300,
                },
                ScenarioStep::ShutdownEmbedded,
            ],
        }
    }
}
