//! [`Script`] SQL row mapping.

use chrono::{DateTime, Utc};
use chronon_core::error::Result;
use chronon_core::models::Script;
use sqlx::{ColumnIndex, Row};

use super::{decode_json, encode_json};
use crate::error_map::map_err;

/// SQL row shape for [`Script`].
pub struct ScriptRow {
    pub(crate) script_id: String,
    pub(crate) script_name: String,
    pub(crate) signature_json: String,
    pub(crate) signature_hash: String,
    pub(crate) created_at: DateTime<Utc>,
}

#[allow(clippy::wrong_self_convention)]
impl ScriptRow {
    /// Build a row from a domain [`Script`].
    pub fn from_model(script: &Script) -> Result<Self> {
        Ok(Self {
            script_id: script.script_id.clone(),
            script_name: script.script_name.clone(),
            signature_json: encode_json(&script.signature_json)?,
            signature_hash: script.signature_hash.clone(),
            created_at: script.created_at,
        })
    }

    /// Convert this row into a domain [`Script`].
    pub fn to_model(self) -> Result<Script> {
        Ok(Script {
            script_id: self.script_id,
            script_name: self.script_name,
            signature_json: decode_json(&self.signature_json)?,
            signature_hash: self.signature_hash,
            created_at: self.created_at,
        })
    }
}

/// Map a SQL row to a [`Script`].
pub fn row_to_script<'r, R>(row: &'r R) -> Result<Script>
where
    R: Row,
    for<'i> &'i str: ColumnIndex<R>,
    String: sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
    DateTime<Utc>: sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
{
    ScriptRow {
        script_id: row.try_get("script_id").map_err(map_err)?,
        script_name: row.try_get("script_name").map_err(map_err)?,
        signature_json: row.try_get("signature_json").map_err(map_err)?,
        signature_hash: row.try_get("signature_hash").map_err(map_err)?,
        created_at: row.try_get("created_at").map_err(map_err)?,
    }
    .to_model()
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use chronon_core::models::Script;
    use serde_json::json;

    use super::ScriptRow;

    #[test]
    fn script_row_roundtrip() {
        let script = Script {
            script_id: "s1".into(),
            script_name: "hello".into(),
            signature_json: json!({"params": []}),
            signature_hash: "abc".into(),
            created_at: Utc::now(),
        };
        let row = ScriptRow::from_model(&script).expect("row");
        let back = row.to_model().expect("model");
        assert_eq!(back.script_name, "hello");
    }
}
