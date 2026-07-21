//! [`Run`] SQL row mapping.

use chrono::{DateTime, Utc};
use chronon_core::error::Result;
use chronon_core::models::Run;
use sqlx::{ColumnIndex, Row};

use super::{
    decode_json, decode_json_opt, encode_json, encode_json_opt, parse_run_status, run_status_to_str,
};
use crate::error_map::map_err;

/// SQL row shape for [`Run`].
pub struct RunRow {
    pub(crate) run_id: String,
    pub(crate) job_id: Option<String>,
    pub(crate) script_name: String,
    pub(crate) parent_run_id: Option<String>,
    pub(crate) root_run_id: Option<String>,
    pub(crate) child_index: Option<i32>,
    pub(crate) scheduled_for: DateTime<Utc>,
    pub(crate) started_at: Option<DateTime<Utc>>,
    pub(crate) finished_at: Option<DateTime<Utc>>,
    pub(crate) duration_ms: Option<i64>,
    pub(crate) status: String,
    pub(crate) attempt: i32,
    pub(crate) instance_id: Option<String>,
    pub(crate) placement_json: Option<String>,
    pub(crate) pool_id: Option<String>,
    pub(crate) actor_json: String,
    pub(crate) params_json: String,
    pub(crate) stdout_text: Option<String>,
    pub(crate) stderr_text: Option<String>,
    pub(crate) error_json: Option<String>,
    pub(crate) stats_json: Option<String>,
    pub(crate) claimed_by: Option<String>,
    pub(crate) claim_lease_until: Option<DateTime<Utc>>,
}

#[allow(clippy::wrong_self_convention)]
impl RunRow {
    /// Build a row from a domain [`Run`].
    pub fn from_model(run: &Run) -> Result<Self> {
        Ok(Self {
            run_id: run.run_id.clone(),
            job_id: run.job_id.clone(),
            script_name: run.script_name.clone(),
            parent_run_id: run.parent_run_id.clone(),
            root_run_id: run.root_run_id.clone(),
            child_index: run.child_index,
            scheduled_for: run.scheduled_for,
            started_at: run.started_at,
            finished_at: run.finished_at,
            duration_ms: run.duration_ms,
            status: run_status_to_str(run.status).to_string(),
            attempt: run.attempt,
            instance_id: run.instance_id.clone(),
            placement_json: encode_json_opt(run.placement_json.as_ref())?,
            pool_id: run.pool_id.clone(),
            actor_json: encode_json(&run.actor_json)?,
            params_json: encode_json(&run.params_json)?,
            stdout_text: run.stdout_text.clone(),
            stderr_text: run.stderr_text.clone(),
            error_json: encode_json_opt(run.error_json.as_ref())?,
            stats_json: encode_json_opt(run.stats_json.as_ref())?,
            claimed_by: run.claimed_by.clone(),
            claim_lease_until: run.claim_lease_until,
        })
    }

    /// Convert this row into a domain [`Run`].
    pub fn to_model(self) -> Result<Run> {
        Ok(Run {
            run_id: self.run_id,
            job_id: self.job_id,
            script_name: self.script_name,
            parent_run_id: self.parent_run_id,
            root_run_id: self.root_run_id,
            child_index: self.child_index,
            scheduled_for: self.scheduled_for,
            started_at: self.started_at,
            finished_at: self.finished_at,
            duration_ms: self.duration_ms,
            status: parse_run_status(&self.status)?,
            attempt: self.attempt,
            instance_id: self.instance_id,
            placement_json: decode_json_opt(self.placement_json)?,
            pool_id: self.pool_id,
            actor_json: decode_json(&self.actor_json)?,
            params_json: decode_json(&self.params_json)?,
            stdout_text: self.stdout_text,
            stderr_text: self.stderr_text,
            error_json: decode_json_opt(self.error_json)?,
            stats_json: decode_json_opt(self.stats_json)?,
            claimed_by: self.claimed_by,
            claim_lease_until: self.claim_lease_until,
        })
    }
}

/// Map a SQL row to a [`Run`].
pub fn row_to_run<'r, R>(row: &'r R) -> Result<Run>
where
    R: Row,
    for<'i> &'i str: ColumnIndex<R>,
    String: sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
    i32: sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
    i64: sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
    DateTime<Utc>: sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
    Option<String>: sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
    Option<i32>: sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
    Option<i64>: sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
    Option<DateTime<Utc>>: sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
{
    let status: String = row.try_get("status").map_err(map_err)?;
    RunRow {
        run_id: row.try_get("run_id").map_err(map_err)?,
        job_id: row.try_get("job_id").map_err(map_err)?,
        script_name: row.try_get("script_name").map_err(map_err)?,
        parent_run_id: row.try_get("parent_run_id").map_err(map_err)?,
        root_run_id: row.try_get("root_run_id").map_err(map_err)?,
        child_index: row.try_get("child_index").map_err(map_err)?,
        scheduled_for: row.try_get("scheduled_for").map_err(map_err)?,
        started_at: row.try_get("started_at").map_err(map_err)?,
        finished_at: row.try_get("finished_at").map_err(map_err)?,
        duration_ms: row.try_get("duration_ms").map_err(map_err)?,
        status,
        attempt: row.try_get("attempt").map_err(map_err)?,
        instance_id: row.try_get("instance_id").map_err(map_err)?,
        placement_json: row.try_get("placement_json").map_err(map_err)?,
        pool_id: row.try_get("pool_id").map_err(map_err)?,
        actor_json: row.try_get("actor_json").map_err(map_err)?,
        params_json: row.try_get("params_json").map_err(map_err)?,
        stdout_text: row.try_get("stdout_text").map_err(map_err)?,
        stderr_text: row.try_get("stderr_text").map_err(map_err)?,
        error_json: row.try_get("error_json").map_err(map_err)?,
        stats_json: row.try_get("stats_json").map_err(map_err)?,
        claimed_by: row.try_get("claimed_by").map_err(map_err)?,
        claim_lease_until: row.try_get("claim_lease_until").map_err(map_err)?,
    }
    .to_model()
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use chronon_core::models::{Run, RunStatus};

    use super::RunRow;

    #[test]
    fn run_row_roundtrip() {
        let run = Run {
            run_id: "r1".into(),
            job_id: Some("j1".into()),
            script_name: "script_a".into(),
            parent_run_id: None,
            root_run_id: None,
            child_index: None,
            scheduled_for: Utc::now(),
            started_at: None,
            finished_at: None,
            duration_ms: None,
            status: RunStatus::Queued,
            attempt: 1,
            instance_id: None,
            placement_json: None,
            pool_id: Some("general".into()),
            actor_json: serde_json::json!({}),
            params_json: serde_json::json!({}),
            stdout_text: None,
            stderr_text: None,
            error_json: None,
            stats_json: None,
            claimed_by: None,
            claim_lease_until: None,
        };
        let row = RunRow::from_model(&run).expect("row");
        let back = row.to_model().expect("model");
        assert_eq!(back.run_id, "r1");
        assert_eq!(back.status, RunStatus::Queued);
    }
}
