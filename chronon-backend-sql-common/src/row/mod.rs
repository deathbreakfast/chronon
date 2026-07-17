//! Row mapping between SQL and [`chronon_core`](chronon_core) DTOs.
//!
//! Internal — used by [`SqlSchedulerStore`](crate::SqlSchedulerStore) query modules.

mod job;
mod leader;
mod partition;
mod pool_key;
mod revision;
mod run;
mod script;
mod worker;

pub use job::{row_to_job, JobRow};
pub use leader::{row_to_leader, SchedulerLeaderRow};
pub use partition::{row_to_partition, PartitionAssignmentRow};
pub use pool_key::run_pool_key;
pub use revision::{row_to_revision, JobRevisionRow};
pub use run::{row_to_run, RunRow};
pub use script::{row_to_script, ScriptRow};
pub use worker::{row_to_worker, WorkerRow};

use chronon_core::error::{ChrononError, Result};
use chronon_core::models::{RunStatus, ScheduleKind, WorkerStatus};
use serde_json::Value;

/// Serialize [`ScheduleKind`] to the snake_case string stored in SQL.
pub fn schedule_kind_to_str(kind: &ScheduleKind) -> &'static str {
    match kind {
        ScheduleKind::Cron => "cron",
        ScheduleKind::RunOnce => "run_once",
        ScheduleKind::Manual => "manual",
    }
}

/// Parse a SQL schedule kind string.
pub fn parse_schedule_kind(s: &str) -> Result<ScheduleKind> {
    match s {
        "cron" => Ok(ScheduleKind::Cron),
        "run_once" => Ok(ScheduleKind::RunOnce),
        "manual" => Ok(ScheduleKind::Manual),
        other => Err(ChrononError::StorageError(format!(
            "unknown schedule kind: {other}"
        ))),
    }
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
        other => Err(ChrononError::StorageError(format!(
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
        other => Err(ChrononError::StorageError(format!(
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

pub(crate) fn decode_json(raw: String) -> Result<Value> {
    Ok(serde_json::from_str(&raw)?)
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
    use chronon_core::models::{RunStatus, ScheduleKind, WorkerStatus};

    use super::{
        parse_run_status, parse_schedule_kind, parse_worker_status, run_status_to_str,
        schedule_kind_to_str, worker_status_to_str,
    };

    #[test]
    fn schedule_kind_roundtrip() {
        for (kind, s) in [
            (ScheduleKind::Cron, "cron"),
            (ScheduleKind::RunOnce, "run_once"),
            (ScheduleKind::Manual, "manual"),
        ] {
            assert_eq!(schedule_kind_to_str(&kind), s);
            assert_eq!(parse_schedule_kind(s).expect("parse"), kind);
        }
    }

    #[test]
    fn schedule_kind_unknown_errors() {
        assert!(matches!(
            parse_schedule_kind("invalid"),
            Err(ChrononError::StorageError(_))
        ));
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
