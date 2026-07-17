//! [`SchedulerLeader`] SQL row mapping.

use chrono::{DateTime, Utc};
use chronon_core::error::Result;
use chronon_core::models::SchedulerLeader;
use sqlx::{ColumnIndex, Row};

use crate::error_map::map_err;

/// SQL row shape for [`SchedulerLeader`].
pub struct SchedulerLeaderRow {
    leader_id: String,
    leader_instance_id: String,
    leader_lease_until: DateTime<Utc>,
    last_heartbeat_at: DateTime<Utc>,
}

#[allow(clippy::wrong_self_convention)]
impl SchedulerLeaderRow {
    /// Build a row from a domain [`SchedulerLeader`].
    pub fn from_model(leader: &SchedulerLeader) -> Self {
        Self {
            leader_id: leader.leader_id.clone(),
            leader_instance_id: leader.leader_instance_id.clone(),
            leader_lease_until: leader.leader_lease_until,
            last_heartbeat_at: leader.last_heartbeat_at,
        }
    }

    /// Convert this row into a domain [`SchedulerLeader`].
    pub fn to_model(self) -> SchedulerLeader {
        SchedulerLeader {
            leader_id: self.leader_id,
            leader_instance_id: self.leader_instance_id,
            leader_lease_until: self.leader_lease_until,
            last_heartbeat_at: self.last_heartbeat_at,
        }
    }
}

/// Map a SQL row to a [`SchedulerLeader`].
pub fn row_to_leader<'r, R>(row: &'r R) -> Result<SchedulerLeader>
where
    R: Row,
    for<'i> &'i str: ColumnIndex<R>,
    String: sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
    DateTime<Utc>: sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
{
    Ok(SchedulerLeaderRow {
        leader_id: row.try_get("leader_id").map_err(|e| map_err(&e))?,
        leader_instance_id: row.try_get("leader_instance_id").map_err(|e| map_err(&e))?,
        leader_lease_until: row.try_get("leader_lease_until").map_err(|e| map_err(&e))?,
        last_heartbeat_at: row.try_get("last_heartbeat_at").map_err(|e| map_err(&e))?,
    }
    .to_model())
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use chronon_core::models::SchedulerLeader;

    use super::SchedulerLeaderRow;

    #[test]
    fn leader_row_roundtrip() {
        let leader = SchedulerLeader {
            leader_id: "singleton".into(),
            leader_instance_id: "inst".into(),
            leader_lease_until: Utc::now(),
            last_heartbeat_at: Utc::now(),
        };
        let row = SchedulerLeaderRow::from_model(&leader);
        assert_eq!(row.to_model().leader_instance_id, "inst");
    }
}
