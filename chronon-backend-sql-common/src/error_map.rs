//! Map `sqlx` errors to [`ChrononError::StorageError`](chronon_core::error::ChrononError::StorageError).

use chronon_core::error::ChrononError;

/// Convert a `sqlx` error into [`ChrononError::StorageError`], preserving the source chain.
pub fn map_err(e: sqlx::Error) -> ChrononError {
    let message = e.to_string();
    ChrononError::storage_source(message, e)
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use chronon_core::error::ChrononError;

    use super::map_err;

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
}
