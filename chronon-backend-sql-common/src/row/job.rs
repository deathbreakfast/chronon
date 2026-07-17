//! [`Job`] SQL row mapping.

use chrono::{DateTime, Utc};
use chronon_core::error::Result;
use chronon_core::models::Job;
use sqlx::{ColumnIndex, Row};

use super::{
    decode_json, decode_json_opt, encode_json, encode_json_opt, parse_schedule_kind,
    schedule_kind_to_str,
};
use crate::error_map::map_err;

/// SQL row shape for [`Job`].
pub struct JobRow {
    pub(crate) job_id: String,
    pub(crate) job_name: String,
    pub(crate) script_name: String,
    pub(crate) script_sig_hash: String,
    pub(crate) enabled: bool,
    pub(crate) schedule_kind: String,
    pub(crate) cron_expr: Option<String>,
    pub(crate) timezone: Option<String>,
    pub(crate) run_once_at: Option<DateTime<Utc>>,
    pub(crate) run_once_claimed_at: Option<DateTime<Utc>>,
    pub(crate) run_once_claimed_by: Option<String>,
    pub(crate) run_once_completed_at: Option<DateTime<Utc>>,
    pub(crate) run_once_claim_expires_at: Option<DateTime<Utc>>,
    pub(crate) partition_hash: Option<i64>,
    pub(crate) claim_lease_id: Option<String>,
    pub(crate) claim_lease_until: Option<DateTime<Utc>>,
    pub(crate) pool: Option<String>,
    pub(crate) region: Option<String>,
    pub(crate) placement_json: Option<String>,
    pub(crate) actor_json: String,
    pub(crate) params_json: String,
    pub(crate) concurrency: i32,
    pub(crate) timeout_ms: Option<i64>,
    pub(crate) retry_policy_json: String,
    pub(crate) misfire_policy_json: String,
    pub(crate) parent_limits_json: Option<String>,
    pub(crate) next_run_at: Option<DateTime<Utc>>,
    pub(crate) current_revision: i32,
    pub(crate) updated_at: DateTime<Utc>,
    pub(crate) created_at: DateTime<Utc>,
}

#[allow(clippy::wrong_self_convention)]
impl JobRow {
    /// Build a row from a domain [`Job`].
    pub fn from_model(job: &Job) -> Result<Self> {
        Ok(Self {
            job_id: job.job_id.clone(),
            job_name: job.job_name.clone(),
            script_name: job.script_name.clone(),
            script_sig_hash: job.script_sig_hash.clone(),
            enabled: job.enabled,
            schedule_kind: schedule_kind_to_str(&job.schedule_kind).to_string(),
            cron_expr: job.cron_expr.clone(),
            timezone: job.timezone.clone(),
            run_once_at: job.run_once_at,
            run_once_claimed_at: job.run_once_claimed_at,
            run_once_claimed_by: job.run_once_claimed_by.clone(),
            run_once_completed_at: job.run_once_completed_at,
            run_once_claim_expires_at: job.run_once_claim_expires_at,
            partition_hash: job.partition_hash,
            claim_lease_id: job.claim_lease_id.clone(),
            claim_lease_until: job.claim_lease_until,
            pool: job.pool.clone(),
            region: job.region.clone(),
            placement_json: encode_json_opt(job.placement_json.as_ref())?,
            actor_json: encode_json(&job.actor_json)?,
            params_json: encode_json(&job.params_json)?,
            concurrency: job.concurrency,
            timeout_ms: job.timeout_ms,
            retry_policy_json: encode_json(&job.retry_policy_json)?,
            misfire_policy_json: encode_json(&job.misfire_policy_json)?,
            parent_limits_json: encode_json_opt(job.parent_limits_json.as_ref())?,
            next_run_at: job.next_run_at,
            current_revision: job.current_revision,
            updated_at: job.updated_at,
            created_at: job.created_at,
        })
    }

    /// Convert this row into a domain [`Job`].
    pub fn to_model(self) -> Result<Job> {
        Ok(Job {
            job_id: self.job_id,
            job_name: self.job_name,
            script_name: self.script_name,
            script_sig_hash: self.script_sig_hash,
            enabled: self.enabled,
            schedule_kind: parse_schedule_kind(&self.schedule_kind)?,
            cron_expr: self.cron_expr,
            timezone: self.timezone,
            run_once_at: self.run_once_at,
            run_once_claimed_at: self.run_once_claimed_at,
            run_once_claimed_by: self.run_once_claimed_by,
            run_once_completed_at: self.run_once_completed_at,
            run_once_claim_expires_at: self.run_once_claim_expires_at,
            partition_hash: self.partition_hash,
            claim_lease_id: self.claim_lease_id,
            claim_lease_until: self.claim_lease_until,
            pool: self.pool,
            region: self.region,
            placement_json: decode_json_opt(self.placement_json)?,
            actor_json: decode_json(self.actor_json)?,
            params_json: decode_json(self.params_json)?,
            concurrency: self.concurrency,
            timeout_ms: self.timeout_ms,
            retry_policy_json: decode_json(self.retry_policy_json)?,
            misfire_policy_json: decode_json(self.misfire_policy_json)?,
            parent_limits_json: decode_json_opt(self.parent_limits_json)?,
            next_run_at: self.next_run_at,
            current_revision: self.current_revision,
            updated_at: self.updated_at,
            created_at: self.created_at,
        })
    }
}

/// Map a SQL row to a [`Job`].
pub fn row_to_job<'r, R>(row: &'r R) -> Result<Job>
where
    R: Row,
    for<'i> &'i str: ColumnIndex<R>,
    String: sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
    i32: sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
    i64: sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
    bool: sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
    DateTime<Utc>: sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
    Option<String>: sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
    Option<i64>: sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
    Option<DateTime<Utc>>: sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
{
    let enabled: bool = if let Ok(b) = row.try_get::<bool, _>("enabled") {
        b
    } else {
        let v: i32 = row.try_get("enabled").map_err(|e| map_err(&e))?;
        v != 0
    };
    let schedule_kind: String = row.try_get("schedule_kind").map_err(|e| map_err(&e))?;
    JobRow {
        job_id: row.try_get("job_id").map_err(|e| map_err(&e))?,
        job_name: row.try_get("job_name").map_err(|e| map_err(&e))?,
        script_name: row.try_get("script_name").map_err(|e| map_err(&e))?,
        script_sig_hash: row.try_get("script_sig_hash").map_err(|e| map_err(&e))?,
        enabled,
        schedule_kind,
        cron_expr: row.try_get("cron_expr").map_err(|e| map_err(&e))?,
        timezone: row.try_get("timezone").map_err(|e| map_err(&e))?,
        run_once_at: row.try_get("run_once_at").map_err(|e| map_err(&e))?,
        run_once_claimed_at: row.try_get("run_once_claimed_at").map_err(|e| map_err(&e))?,
        run_once_claimed_by: row.try_get("run_once_claimed_by").map_err(|e| map_err(&e))?,
        run_once_completed_at: row.try_get("run_once_completed_at").map_err(|e| map_err(&e))?,
        run_once_claim_expires_at: row
            .try_get("run_once_claim_expires_at")
            .map_err(|e| map_err(&e))?,
        partition_hash: row.try_get("partition_hash").map_err(|e| map_err(&e))?,
        claim_lease_id: row.try_get("claim_lease_id").map_err(|e| map_err(&e))?,
        claim_lease_until: row.try_get("claim_lease_until").map_err(|e| map_err(&e))?,
        pool: row.try_get("pool").map_err(|e| map_err(&e))?,
        region: row.try_get("region").map_err(|e| map_err(&e))?,
        placement_json: row.try_get("placement_json").map_err(|e| map_err(&e))?,
        actor_json: row.try_get("actor_json").map_err(|e| map_err(&e))?,
        params_json: row.try_get("params_json").map_err(|e| map_err(&e))?,
        concurrency: row.try_get("concurrency").map_err(|e| map_err(&e))?,
        timeout_ms: row.try_get("timeout_ms").map_err(|e| map_err(&e))?,
        retry_policy_json: row.try_get("retry_policy_json").map_err(|e| map_err(&e))?,
        misfire_policy_json: row.try_get("misfire_policy_json").map_err(|e| map_err(&e))?,
        parent_limits_json: row.try_get("parent_limits_json").map_err(|e| map_err(&e))?,
        next_run_at: row.try_get("next_run_at").map_err(|e| map_err(&e))?,
        current_revision: row.try_get("current_revision").map_err(|e| map_err(&e))?,
        updated_at: row.try_get("updated_at").map_err(|e| map_err(&e))?,
        created_at: row.try_get("created_at").map_err(|e| map_err(&e))?,
    }
    .to_model()
}

#[cfg(test)]
mod tests {
    use chronon_core::models::{Job, ScheduleKind};

    use super::JobRow;

    #[test]
    fn job_row_roundtrip() {
        let job = Job::new("smoke", "script_a");
        let row = JobRow::from_model(&job).expect("row");
        let back = row.to_model().expect("model");
        assert_eq!(back.job_name, "smoke");
        assert_eq!(back.schedule_kind, ScheduleKind::Cron);
    }
}
