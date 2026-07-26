//! Map `sqlx` errors to [`ChrononError::StorageError`](chronon_core::error::ChrononError::StorageError).

use chronon_core::error::ChrononError;
use chronon_core::{redact_credentials_in_text, redact_endpoint};

/// Convert a `sqlx` error into [`ChrononError::StorageError`], preserving the source chain.
///
/// Display text redacts URL userinfo when present.
pub fn map_err(e: sqlx::Error) -> ChrononError {
    let message = redact_credentials_in_text(&e.to_string());
    ChrononError::storage_source(message, e)
}

/// Connect failure labeled with a redacted endpoint (Postgres / SQLite).
pub fn map_connect_err(kind: &str, url: &str, e: sqlx::Error) -> ChrononError {
    let detail = redact_credentials_in_text(&e.to_string());
    ChrononError::storage_source(
        format!("{kind} connect {}: {detail}", redact_endpoint(url)),
        e,
    )
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use chronon_core::error::ChrononError;

    use super::{map_connect_err, map_err};

    #[test]
    fn map_err_wraps_message_and_source() {
        let err = sqlx::Error::PoolTimedOut;
        let mapped = map_err(err);
        assert!(matches!(
            mapped,
            ChrononError::StorageError { ref message, source: Some(_) } if message.contains("timed out")
        ));
        assert!(mapped.source().is_some());
    }

    #[test]
    fn map_connect_err_redacts_userinfo() {
        let err = sqlx::Error::PoolTimedOut;
        let mapped = map_connect_err("postgres", "postgres://user:secret@host/db", err);
        let ChrononError::StorageError { message, .. } = mapped else {
            panic!("expected StorageError");
        };
        assert!(message.contains("postgres://***@host/db"));
        assert!(!message.contains("secret"));
    }
}
