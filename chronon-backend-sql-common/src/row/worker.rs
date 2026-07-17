//! [`Worker`] SQL row mapping.

use chrono::{DateTime, Utc};
use chronon_core::error::Result;
use chronon_core::models::Worker;
use sqlx::{ColumnIndex, Row};

use super::{decode_json_opt, encode_json_opt, parse_worker_status, worker_status_to_str};
use crate::error_map::map_err;

/// SQL row shape for [`Worker`].
pub struct WorkerRow {
    pub(crate) worker_id: String,
    pub(crate) pool_id: String,
    pub(crate) cell_id: Option<String>,
    pub(crate) status: String,
    pub(crate) last_heartbeat_at: DateTime<Utc>,
    pub(crate) capacity_json: Option<String>,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
}

#[allow(clippy::wrong_self_convention)]
impl WorkerRow {
    /// Build a row from a domain [`Worker`].
    pub fn from_model(worker: &Worker) -> Result<Self> {
        Ok(Self {
            worker_id: worker.worker_id.clone(),
            pool_id: worker.pool_id.clone(),
            cell_id: worker.cell_id.clone(),
            status: worker_status_to_str(worker.status).to_string(),
            last_heartbeat_at: worker.last_heartbeat_at,
            capacity_json: encode_json_opt(worker.capacity_json.as_ref())?,
            created_at: worker.created_at,
            updated_at: worker.updated_at,
        })
    }

    /// Convert this row into a domain [`Worker`].
    pub fn to_model(self) -> Result<Worker> {
        Ok(Worker {
            worker_id: self.worker_id,
            pool_id: self.pool_id,
            cell_id: self.cell_id,
            status: parse_worker_status(&self.status)?,
            last_heartbeat_at: self.last_heartbeat_at,
            capacity_json: decode_json_opt(self.capacity_json)?,
            created_at: self.created_at,
            updated_at: self.updated_at,
        })
    }
}

/// Map a SQL row to a [`Worker`].
pub fn row_to_worker<'r, R>(row: &'r R) -> Result<Worker>
where
    R: Row,
    for<'i> &'i str: ColumnIndex<R>,
    String: sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
    DateTime<Utc>: sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
    Option<String>: sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
{
    let status: String = row.try_get("status").map_err(|e| map_err(&e))?;
    WorkerRow {
        worker_id: row.try_get("worker_id").map_err(|e| map_err(&e))?,
        pool_id: row.try_get("pool_id").map_err(|e| map_err(&e))?,
        cell_id: row.try_get("cell_id").map_err(|e| map_err(&e))?,
        status,
        last_heartbeat_at: row.try_get("last_heartbeat_at").map_err(|e| map_err(&e))?,
        capacity_json: row.try_get("capacity_json").map_err(|e| map_err(&e))?,
        created_at: row.try_get("created_at").map_err(|e| map_err(&e))?,
        updated_at: row.try_get("updated_at").map_err(|e| map_err(&e))?,
    }
    .to_model()
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use chronon_core::models::{Worker, WorkerStatus};

    use super::WorkerRow;

    #[test]
    fn worker_row_roundtrip() {
        let worker = Worker {
            worker_id: "w1".into(),
            pool_id: "general".into(),
            cell_id: None,
            status: WorkerStatus::Online,
            last_heartbeat_at: Utc::now(),
            capacity_json: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let row = WorkerRow::from_model(&worker).expect("row");
        let back = row.to_model().expect("model");
        assert_eq!(back.worker_id, "w1");
    }
}
