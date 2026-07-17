//! [`PartitionAssignment`] SQL row mapping.

use chrono::{DateTime, Utc};
use chronon_core::error::Result;
use chronon_core::models::PartitionAssignment;
use sqlx::{ColumnIndex, Row};

use crate::error_map::map_err;

/// SQL row shape for [`PartitionAssignment`].
pub struct PartitionAssignmentRow {
    pub(crate) partition_id: String,
    pub(crate) owner_instance_id: String,
    pub(crate) lease_until: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
}

#[allow(clippy::wrong_self_convention)]
impl PartitionAssignmentRow {
    /// Build a row from a domain [`PartitionAssignment`].
    pub fn from_model(assignment: &PartitionAssignment) -> Self {
        Self {
            partition_id: assignment.partition_id.clone(),
            owner_instance_id: assignment.owner_instance_id.clone(),
            lease_until: assignment.lease_until,
            updated_at: assignment.updated_at,
        }
    }

    /// Convert this row into a domain [`PartitionAssignment`].
    pub fn to_model(self) -> PartitionAssignment {
        PartitionAssignment {
            partition_id: self.partition_id,
            owner_instance_id: self.owner_instance_id,
            lease_until: self.lease_until,
            updated_at: self.updated_at,
        }
    }
}

/// Map a SQL row to a [`PartitionAssignment`].
pub fn row_to_partition<'r, R>(row: &'r R) -> Result<PartitionAssignment>
where
    R: Row,
    for<'i> &'i str: ColumnIndex<R>,
    String: sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
    DateTime<Utc>: sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
{
    Ok(PartitionAssignmentRow {
        partition_id: row.try_get("partition_id").map_err(|e| map_err(&e))?,
        owner_instance_id: row.try_get("owner_instance_id").map_err(|e| map_err(&e))?,
        lease_until: row.try_get("lease_until").map_err(|e| map_err(&e))?,
        updated_at: row.try_get("updated_at").map_err(|e| map_err(&e))?,
    }
    .to_model())
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use chronon_core::models::PartitionAssignment;

    use super::PartitionAssignmentRow;

    #[test]
    fn partition_row_roundtrip() {
        let assignment = PartitionAssignment {
            partition_id: "p0".into(),
            owner_instance_id: "inst".into(),
            lease_until: Utc::now(),
            updated_at: Utc::now(),
        };
        let row = PartitionAssignmentRow::from_model(&assignment);
        assert_eq!(row.to_model().partition_id, "p0");
    }
}
