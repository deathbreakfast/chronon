//! [`JobRevision`] SQL row mapping.

use chrono::{DateTime, Utc};
use chronon_core::error::Result;
use chronon_core::models::JobRevision;
use sqlx::{ColumnIndex, Row};

use super::{decode_json, encode_json};
use crate::error_map::map_err;

/// SQL row shape for [`JobRevision`].
pub struct JobRevisionRow {
    pub(crate) revision_id: String,
    pub(crate) job_id: String,
    pub(crate) revision_number: i32,
    pub(crate) changed_at: DateTime<Utc>,
    pub(crate) changed_by_actor_json: String,
    pub(crate) snapshot_json: String,
}

#[allow(clippy::wrong_self_convention)]
impl JobRevisionRow {
    /// Build a row from a domain [`JobRevision`].
    pub fn from_model(revision: &JobRevision) -> Result<Self> {
        Ok(Self {
            revision_id: revision.revision_id.clone(),
            job_id: revision.job_id.clone(),
            revision_number: revision.revision_number,
            changed_at: revision.changed_at,
            changed_by_actor_json: encode_json(&revision.changed_by_actor_json)?,
            snapshot_json: encode_json(&revision.snapshot_json)?,
        })
    }

    /// Convert this row into a domain [`JobRevision`].
    pub fn to_model(self) -> Result<JobRevision> {
        Ok(JobRevision {
            revision_id: self.revision_id,
            job_id: self.job_id,
            revision_number: self.revision_number,
            changed_at: self.changed_at,
            changed_by_actor_json: decode_json(&self.changed_by_actor_json)?,
            snapshot_json: decode_json(&self.snapshot_json)?,
        })
    }
}

/// Map a SQL row to a [`JobRevision`].
pub fn row_to_revision<'r, R>(row: &'r R) -> Result<JobRevision>
where
    R: Row,
    for<'i> &'i str: ColumnIndex<R>,
    String: sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
    i32: sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
    DateTime<Utc>: sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
{
    JobRevisionRow {
        revision_id: row.try_get("revision_id").map_err(map_err)?,
        job_id: row.try_get("job_id").map_err(map_err)?,
        revision_number: row.try_get("revision_number").map_err(map_err)?,
        changed_at: row.try_get("changed_at").map_err(map_err)?,
        changed_by_actor_json: row.try_get("changed_by_actor_json").map_err(map_err)?,
        snapshot_json: row.try_get("snapshot_json").map_err(map_err)?,
    }
    .to_model()
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use chronon_core::models::JobRevision;
    use serde_json::json;

    use super::JobRevisionRow;

    #[test]
    fn revision_row_roundtrip() {
        let revision = JobRevision {
            revision_id: "rev1".into(),
            job_id: "job1".into(),
            revision_number: 2,
            changed_at: Utc::now(),
            changed_by_actor_json: json!({"actor": "test"}),
            snapshot_json: json!({"enabled": true}),
        };
        let row = JobRevisionRow::from_model(&revision).expect("row");
        let back = row.to_model().expect("model");
        assert_eq!(back.revision_number, 2);
    }
}
