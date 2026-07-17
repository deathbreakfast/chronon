//! Error types for Chronon.

use thiserror::Error;

/// Result type alias for Chronon operations.
pub type Result<T> = std::result::Result<T, ChrononError>;

/// Errors that can occur in Chronon operations.
///
/// Returned by [`SchedulerStore`](crate::store::SchedulerStore) implementations, the runtime
/// builder, and script dispatch. Hosts typically map these to HTTP status codes or log events.
#[derive(Debug, Error)]
pub enum ChrononError {
    /// No script with the requested name is registered or persisted.
    #[error("script not found: {0}")]
    ScriptNotFound(String),

    /// No job with the requested id or name exists in storage.
    #[error("job not found: {0}")]
    JobNotFound(String),

    /// No run with the requested id exists in storage.
    #[error("run not found: {0}")]
    RunNotFound(String),

    /// Cron expression failed validation (syntax or unsupported field).
    #[error("invalid cron expression: {0}")]
    InvalidCron(String),

    /// IANA timezone string could not be parsed.
    #[error("invalid timezone: {0}")]
    InvalidTimezone(String),

    /// Job parameters, actor JSON, or handler inputs failed validation or deserialization.
    #[error("parameter error: {0}")]
    ParamError(String),

    /// Job references a script name that does not match the registered script identity.
    #[error("script mismatch for job '{job_name}': expected '{expected}', got '{actual}'")]
    ScriptMismatch {
        /// Script name recorded on the job revision.
        expected: String,
        /// Script name resolved from the live registry or request.
        actual: String,
        /// Human-readable job name for error messages.
        job_name: String,
    },

    /// Underlying storage backend failed or returned an unexpected condition.
    #[error("storage error: {0}")]
    StorageError(String),

    /// Catch-all for invariant violations, identity reconstruction failures, and bugs.
    #[error("internal error: {0}")]
    Internal(String),
}

impl From<serde_json::Error> for ChrononError {
    fn from(err: serde_json::Error) -> Self {
        Self::ParamError(err.to_string())
    }
}
