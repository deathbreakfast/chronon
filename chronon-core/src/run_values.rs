//! Run values: the facts about a run that a script can read and a job can bind into params.
//!
//! A run's values are stored as [`Run::run_values_json`] so a retry sees the same values. The
//! worker decodes them before invoking the script and hands them over through
//! [`ScriptContext::run`](crate::ScriptContext::run). There is no slot or worker number here: those
//! change between retries and workers.
//!
//! # Examples
//!
//! ```
//! use chrono::Utc;
//! use chronon_core::{Job, Run, RunTrigger, RunValues};
//!
//! let job = Job::new("nightly", "cleanup");
//! let run = Run::for_job(&job.job_id, "cleanup", Utc::now());
//! let values = RunValues::from_run(&run, &job);
//! assert_eq!(values.run.id, run.run_id);
//! assert_eq!(values.run.trigger, RunTrigger::Cron);
//! assert!(values.child.is_none());
//! ```

use std::sync::LazyLock;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::models::{Job, Run, ScheduleKind};

/// What caused a run to be enqueued.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunTrigger {
    /// A cron tick.
    Cron,
    /// A run-once tick.
    RunOnce,
    /// `run_now` (in-process or HTTP).
    Manual,
    /// Spawned as a child of another run.
    Child,
    /// Handed out as one shard of a sharded job.
    Shard,
    /// A retry of a failed attempt.
    Retry,
}

/// The run itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunInfo {
    /// [`Run::run_id`].
    pub id: String,
    /// Attempt number, starting at 1.
    pub attempt: u32,
    /// When the run was scheduled to execute.
    pub scheduled_for: DateTime<Utc>,
    /// When execution started, once known.
    #[serde(default)]
    pub started_at: Option<DateTime<Utc>>,
    /// What enqueued the run.
    pub trigger: RunTrigger,
}

/// The job the run belongs to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobInfo {
    /// [`Job::job_id`].
    pub id: String,
    /// [`Job::job_name`].
    pub name: String,
    /// [`Job::current_revision`] when the run was created.
    pub revision: u32,
    /// [`Job::concurrency`] when the run was created.
    pub concurrency: u32,
}

/// The script the run executes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptInfo {
    /// Registered script name.
    pub name: String,
}

/// The top of the run's lineage tree (the run itself for top-level runs).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RootInfo {
    /// Root run id.
    pub run_id: String,
    /// Root run's job name.
    pub job_name: String,
}

/// The run that spawned this one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParentValues {
    /// Parent run id.
    pub run_id: String,
    /// Parent job id.
    pub job_id: String,
    /// Parent job name.
    pub job_name: String,
    /// Parent attempt number.
    pub attempt: u32,
    /// When the parent run was scheduled.
    pub scheduled_for: DateTime<Utc>,
    /// Parent run's params.
    #[serde(default)]
    pub params: Map<String, Value>,
    /// Values the parent published for its children ([`Run::outputs_json`]).
    #[serde(default)]
    pub output: Map<String, Value>,
}

/// This run's place in its [`ChildSet`](crate::ChildSet).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChildValues {
    /// Zero-based position in the set.
    pub index: u32,
    /// One-based position in the set (`index + 1`).
    pub number: u32,
    /// Number of children in the set.
    pub count: u32,
    /// Key the parent gave this child, if any.
    #[serde(default)]
    pub key: Option<String>,
    /// Item the parent gave this child, if any.
    #[serde(default)]
    pub item: Option<Value>,
    /// [`ChildSet::set_id`](crate::ChildSet::set_id).
    pub set_id: String,
}

/// This run's range in its [`ShardSet`](crate::ShardSet).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShardValues {
    /// Zero-based shard index.
    pub index: u32,
    /// One-based shard number (`index + 1`).
    pub number: u32,
    /// Number of shards, when the end of the walk is known.
    #[serde(default)]
    pub count: Option<u32>,
    /// Field the cursor walks.
    pub cursor_field: String,
    /// First cursor value in the range (inclusive).
    pub start: Value,
    /// End of the range (exclusive).
    pub end: Value,
    /// Rows the shard covers.
    pub size: u32,
    /// Where to resume: the last checkpoint, or `start` on a first attempt.
    pub resume_from: Value,
}

/// Every run value a script can read through [`ScriptContext::run`](crate::ScriptContext::run).
///
/// Stored on [`Run::run_values_json`]. `parent`, `child` and `shard` are `None` for a top-level
/// run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunValues {
    /// The run itself.
    pub run: RunInfo,
    /// Its job.
    pub job: JobInfo,
    /// Its script.
    pub script: ScriptInfo,
    /// Top of its lineage tree.
    pub root: RootInfo,
    /// Distance from the root (0 for a top-level run).
    #[serde(default)]
    pub depth: u32,
    /// The run that spawned it.
    #[serde(default)]
    pub parent: Option<ParentValues>,
    /// Its place in a child set.
    #[serde(default)]
    pub child: Option<ChildValues>,
    /// Its range in a shard set.
    #[serde(default)]
    pub shard: Option<ShardValues>,
}

static DETACHED: LazyLock<RunValues> = LazyLock::new(|| RunValues {
    run: RunInfo {
        id: String::new(),
        attempt: 1,
        scheduled_for: DateTime::UNIX_EPOCH,
        started_at: None,
        trigger: RunTrigger::Manual,
    },
    job: JobInfo {
        id: String::new(),
        name: String::new(),
        revision: 0,
        concurrency: 1,
    },
    script: ScriptInfo {
        name: String::new(),
    },
    root: RootInfo {
        run_id: String::new(),
        job_name: String::new(),
    },
    depth: 0,
    parent: None,
    child: None,
    shard: None,
});

impl RunValues {
    /// Empty top-level values for a context that is not running inside a Chronon run.
    ///
    /// The default [`ScriptContext::run`](crate::ScriptContext::run) returns this, so host contexts
    /// written before run values existed keep compiling.
    pub fn detached() -> &'static Self {
        &DETACHED
    }

    /// Top-level values derived from the run row and its job.
    ///
    /// Used when [`Run::run_values_json`] is `{}` (runs created before run values were stored).
    /// The trigger is [`RunTrigger::Retry`] for attempts after the first, otherwise the job's
    /// schedule kind.
    pub fn from_run(run: &Run, job: &Job) -> Self {
        let trigger = if run.attempt > 1 {
            RunTrigger::Retry
        } else {
            match job.schedule_kind {
                ScheduleKind::Cron => RunTrigger::Cron,
                ScheduleKind::RunOnce => RunTrigger::RunOnce,
                ScheduleKind::Manual => RunTrigger::Manual,
                ScheduleKind::Child { .. } => RunTrigger::Child,
            }
        };
        Self {
            run: RunInfo {
                id: run.run_id.clone(),
                attempt: u32::try_from(run.attempt).unwrap_or(1),
                scheduled_for: run.scheduled_for,
                started_at: run.started_at,
                trigger,
            },
            job: JobInfo {
                id: job.job_id.clone(),
                name: job.job_name.clone(),
                revision: u32::try_from(job.current_revision).unwrap_or(0),
                concurrency: u32::try_from(job.concurrency).unwrap_or(1),
            },
            script: ScriptInfo {
                name: run.script_name.clone(),
            },
            root: RootInfo {
                run_id: run
                    .root_run_id
                    .clone()
                    .unwrap_or_else(|| run.run_id.clone()),
                job_name: job.job_name.clone(),
            },
            depth: u32::from(run.parent_run_id.is_some()),
            parent: None,
            child: None,
            shard: None,
        }
    }

    /// Decode the values stored on `run`, falling back to [`Self::from_run`] when they are `{}`.
    ///
    /// # Errors
    ///
    /// Returns [`ChrononError::ParamError`](crate::ChrononError::ParamError) when
    /// `run_values_json` is neither `{}` nor a valid [`RunValues`].
    pub fn for_run(run: &Run, job: &Job) -> crate::Result<Self> {
        match &run.run_values_json {
            Value::Object(map) if map.is_empty() => Ok(Self::from_run(run, job)),
            value => serde_json::from_value(value.clone()).map_err(|e| {
                crate::ChrononError::ParamError(format!(
                    "run {} has invalid run_values_json: {e}",
                    run.run_id
                ))
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn detached_is_empty_top_level() {
        let values = RunValues::detached();
        assert_eq!(values.run.id, "");
        assert_eq!(values.depth, 0);
        assert!(values.parent.is_none() && values.child.is_none() && values.shard.is_none());
    }

    #[test]
    fn empty_json_falls_back_to_run_row() {
        let job = Job::new("j", "s");
        let mut run = Run::for_job(&job.job_id, "s", Utc::now());
        run.attempt = 2;
        let values = RunValues::for_run(&run, &job).unwrap();
        assert_eq!(values.run.id, run.run_id);
        assert_eq!(values.run.attempt, 2);
        assert_eq!(values.run.trigger, RunTrigger::Retry);
        assert_eq!(values.job.id, job.job_id);
        assert_eq!(values.root.run_id, run.run_id);
        assert_eq!(values.script.name, "s");
    }

    #[test]
    fn stored_values_roundtrip() {
        let job = Job::new("j", "s");
        let mut run = Run::for_job(&job.job_id, "s", Utc::now());
        let mut values = RunValues::from_run(&run, &job);
        values.run.trigger = RunTrigger::Child;
        values.depth = 1;
        values.child = Some(ChildValues {
            index: 2,
            number: 3,
            count: 5,
            key: Some("k".into()),
            item: Some(json!({"id": 7})),
            set_id: "set".into(),
        });
        values.shard = Some(ShardValues {
            index: 0,
            number: 1,
            count: None,
            cursor_field: "id".into(),
            start: json!(0),
            end: json!(100),
            size: 100,
            resume_from: json!(0),
        });
        run.run_values_json = serde_json::to_value(&values).unwrap();
        assert_eq!(RunValues::for_run(&run, &job).unwrap(), values);
    }

    #[test]
    fn invalid_values_are_a_param_error() {
        let job = Job::new("j", "s");
        let mut run = Run::for_job(&job.job_id, "s", Utc::now());
        run.run_values_json = json!({"run": 5});
        assert!(matches!(
            RunValues::for_run(&run, &job),
            Err(crate::ChrononError::ParamError(_))
        ));
    }
}
