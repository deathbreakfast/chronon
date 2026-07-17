//! Map `sqlx` errors to [`ChrononError::StorageError`](chronon_core::error::ChrononError::StorageError).

use chronon_core::error::ChrononError;

/// Convert a `sqlx` error into [`ChrononError::StorageError`].
pub fn map_err(e: &sqlx::Error) -> ChrononError {
    ChrononError::StorageError(e.to_string())
}

#[cfg(test)]
mod tests {
    use chronon_core::error::ChrononError;

    use super::map_err;

    #[test]
    fn map_err_wraps_message() {
        let err = sqlx::Error::PoolTimedOut;
        let mapped = map_err(&err);
        assert!(matches!(mapped, ChrononError::StorageError(_)));
        assert!(mapped.to_string().contains("timed out"));
    }
}
