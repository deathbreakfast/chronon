//! Row mapping between SQL and [`chronon_core`](chronon_core) DTOs.
//!
//! Internal — used by [`SqlSchedulerStore`](crate::SqlSchedulerStore) query modules.

mod child_set;
mod job;
mod leader;
mod partition;
mod pool_key;
mod revision;
mod run;
mod script;
mod shard_set;
mod worker;

pub use child_set::row_to_child_set;
pub use job::{row_to_job, JobRow};
pub use leader::{row_to_leader, SchedulerLeaderRow};
pub use partition::{row_to_partition, PartitionAssignmentRow};
pub use pool_key::run_pool_key;
pub use revision::{row_to_revision, JobRevisionRow};
pub use run::{row_to_run, RunRow};
pub use script::{row_to_script, ScriptRow};
pub use shard_set::row_to_shard_set;
pub use worker::{row_to_worker, WorkerRow};

use chronon_core::error::{ChrononError, Result};
use chronon_core::models::{OnBusy, RunStatus, ScheduleKind, WorkerStatus};
use serde_json::Value;

/// Serialize [`ScheduleKind`] to the snake_case string stored in SQL.
pub fn schedule_kind_to_str(kind: &ScheduleKind) -> &'static str {
    match kind {
        ScheduleKind::Cron => "cron",
        ScheduleKind::RunOnce => "run_once",
        ScheduleKind::Manual => "manual",
        ScheduleKind::Child { .. } => "child",
    }
}

/// Parent job id stored in the `parent_job_id` column for [`ScheduleKind::Child`].
pub fn schedule_kind_parent(kind: &ScheduleKind) -> Option<String> {
    match kind {
        ScheduleKind::Child { parent_job_id } => Some(parent_job_id.clone()),
        ScheduleKind::Cron | ScheduleKind::RunOnce | ScheduleKind::Manual => None,
    }
}

/// Parse a SQL schedule kind string; `child` needs the `parent_job_id` column.
pub fn parse_schedule_kind(s: &str, parent_job_id: Option<String>) -> Result<ScheduleKind> {
    match s {
        "cron" => Ok(ScheduleKind::Cron),
        "run_once" => Ok(ScheduleKind::RunOnce),
        "manual" => Ok(ScheduleKind::Manual),
        "child" => parent_job_id
            .map(|parent_job_id| ScheduleKind::Child { parent_job_id })
            .ok_or_else(|| ChrononError::storage("child job without parent_job_id")),
        other => Err(ChrononError::storage(format!(
            "unknown schedule kind: {other}"
        ))),
    }
}

/// Serialize [`OnBusy`] to the snake_case string stored in SQL.
pub const fn on_busy_to_str(on_busy: OnBusy) -> &'static str {
    match on_busy {
        OnBusy::Skip => "skip",
        OnBusy::QueueOne => "queue_one",
        OnBusy::Queue => "queue",
    }
}

/// Parse a SQL on-busy string.
pub fn parse_on_busy(s: &str) -> Result<OnBusy> {
    match s {
        "skip" => Ok(OnBusy::Skip),
        "queue_one" => Ok(OnBusy::QueueOne),
        "queue" => Ok(OnBusy::Queue),
        other => Err(ChrononError::storage(format!("unknown on_busy: {other}"))),
    }
}

/// Convert a stored counter to `u32`, rejecting negative or oversized values.
pub(crate) fn decode_u32(column: &str, value: i64) -> Result<u32> {
    u32::try_from(value)
        .map_err(|_| ChrononError::storage(format!("{column} out of range: {value}")))
}

/// Serialize [`RunStatus`] to the lowercase string stored in SQL.
pub const fn run_status_to_str(status: RunStatus) -> &'static str {
    match status {
        RunStatus::Queued => "queued",
        RunStatus::Claimed => "claimed",
        RunStatus::Running => "running",
        RunStatus::Success => "success",
        RunStatus::Failed => "failed",
        RunStatus::Canceled => "canceled",
        RunStatus::Timeout => "timeout",
    }
}

/// Parse a SQL run status string.
pub fn parse_run_status(s: &str) -> Result<RunStatus> {
    match s {
        "queued" => Ok(RunStatus::Queued),
        "claimed" => Ok(RunStatus::Claimed),
        "running" => Ok(RunStatus::Running),
        "success" => Ok(RunStatus::Success),
        "failed" => Ok(RunStatus::Failed),
        "canceled" => Ok(RunStatus::Canceled),
        "timeout" => Ok(RunStatus::Timeout),
        other => Err(ChrononError::storage(format!(
            "unknown run status: {other}"
        ))),
    }
}

/// Serialize [`WorkerStatus`] to the lowercase string stored in SQL.
pub const fn worker_status_to_str(status: WorkerStatus) -> &'static str {
    match status {
        WorkerStatus::Online => "online",
        WorkerStatus::Draining => "draining",
        WorkerStatus::Offline => "offline",
    }
}

/// Parse a SQL worker status string.
pub fn parse_worker_status(s: &str) -> Result<WorkerStatus> {
    match s {
        "online" => Ok(WorkerStatus::Online),
        "draining" => Ok(WorkerStatus::Draining),
        "offline" => Ok(WorkerStatus::Offline),
        other => Err(ChrononError::storage(format!(
            "unknown worker status: {other}"
        ))),
    }
}

pub(crate) fn decode_json_opt(raw: Option<String>) -> Result<Option<Value>> {
    match raw {
        None => Ok(None),
        Some(s) => Ok(Some(serde_json::from_str(&s)?)),
    }
}

pub(crate) fn decode_json(raw: &str) -> Result<Value> {
    Ok(serde_json::from_str(raw)?)
}

pub(crate) fn encode_json(value: &Value) -> Result<String> {
    Ok(serde_json::to_string(value)?)
}

pub(crate) fn encode_json_opt(value: Option<&Value>) -> Result<Option<String>> {
    match value {
        None => Ok(None),
        Some(v) => Ok(Some(encode_json(v)?)),
    }
}

#[cfg(test)]
mod tests {
    use chronon_core::error::ChrononError;
    use chronon_core::models::{OnBusy, RunStatus, ScheduleKind, WorkerStatus};

    use super::{
        on_busy_to_str, parse_on_busy, parse_run_status, parse_schedule_kind, parse_worker_status,
        run_status_to_str, schedule_kind_parent, schedule_kind_to_str, worker_status_to_str,
    };

    #[test]
    fn schedule_kind_roundtrip() {
        for (kind, s) in [
            (ScheduleKind::Cron, "cron"),
            (ScheduleKind::RunOnce, "run_once"),
            (ScheduleKind::Manual, "manual"),
            (
                ScheduleKind::Child {
                    parent_job_id: "p1".into(),
                },
                "child",
            ),
        ] {
            assert_eq!(schedule_kind_to_str(&kind), s);
            let parent = schedule_kind_parent(&kind);
            assert_eq!(parse_schedule_kind(s, parent).expect("parse"), kind);
        }
    }

    #[test]
    fn schedule_kind_unknown_errors() {
        assert!(matches!(
            parse_schedule_kind("invalid", None),
            Err(ChrononError::StorageError { .. })
        ));
        assert!(matches!(
            parse_schedule_kind("child", None),
            Err(ChrononError::StorageError { .. })
        ));
    }

    #[test]
    fn on_busy_roundtrip() {
        for on_busy in [OnBusy::Skip, OnBusy::QueueOne, OnBusy::Queue] {
            assert_eq!(
                parse_on_busy(on_busy_to_str(on_busy)).expect("parse"),
                on_busy
            );
        }
        assert!(parse_on_busy("nope").is_err());
    }

    #[test]
    fn run_status_roundtrip() {
        for status in [
            RunStatus::Queued,
            RunStatus::Claimed,
            RunStatus::Running,
            RunStatus::Success,
            RunStatus::Failed,
            RunStatus::Canceled,
            RunStatus::Timeout,
        ] {
            let s = run_status_to_str(status);
            assert_eq!(parse_run_status(s).expect("parse"), status);
        }
    }

    #[test]
    fn worker_status_roundtrip() {
        for status in [
            WorkerStatus::Online,
            WorkerStatus::Draining,
            WorkerStatus::Offline,
        ] {
            let s = worker_status_to_str(status);
            assert_eq!(parse_worker_status(s).expect("parse"), status);
        }
    }
}
