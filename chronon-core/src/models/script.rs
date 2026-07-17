//! Script model - represents a registered script in the database.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A registered script in the database.
///
/// Scripts are auto-discovered via `#[chronon::script]` and persisted
/// for reference by jobs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Script {
    /// Unique identifier (UUID).
    pub script_id: String,

    /// Unique stable name from the macro attribute.
    pub script_name: String,

    /// JSON schema for parameters (excluding context).
    pub signature_json: Value,

    /// Hash of signature_json for version checking.
    pub signature_hash: String,

    /// When the script was first registered.
    pub created_at: DateTime<Utc>,
}

impl Script {
    /// Create a new script record.
    pub fn new(
        script_name: impl Into<String>,
        signature_json: Value,
        signature_hash: String,
    ) -> Self {
        Self {
            script_id: uuid::Uuid::new_v4().to_string(),
            script_name: script_name.into(),
            signature_json,
            signature_hash,
            created_at: Utc::now(),
        }
    }
}
