//! Test helpers for PostgreSQL scheduler store integration.

use chronon_core::Result;

use crate::PostgresSchedulerStore;

/// Resolve a PostgreSQL URL for tests.
///
/// Checks `CHRONON_POSTGRES_URL`, then `CHRONON_TEST_POSTGRES_URL`, then
/// `postgres://localhost/chronon_test`.
#[must_use]
pub fn postgres_test_url() -> String {
    std::env::var("CHRONON_POSTGRES_URL")
        .or_else(|_| std::env::var("CHRONON_TEST_POSTGRES_URL"))
        .unwrap_or_else(|_| "postgres://localhost/chronon_test".into())
}

/// Connect using `CHRONON_POSTGRES_SCHEMA` when set (isolated schema for multi-process E2E).
///
/// When `CHRONON_POSTGRES_SKIP_BOOTSTRAP` is set, attaches to an existing schema without DDL.
///
/// # Errors
///
/// Returns a storage error when the pool cannot be opened or schema bootstrap fails.
pub async fn postgres_store_from_env() -> Result<PostgresSchedulerStore> {
    let url = postgres_test_url();
    if let Ok(schema) = std::env::var("CHRONON_POSTGRES_SCHEMA") {
        if std::env::var("CHRONON_POSTGRES_SKIP_BOOTSTRAP").is_ok() {
            PostgresSchedulerStore::attach_isolated(&url, &schema).await
        } else {
            PostgresSchedulerStore::connect_isolated(&url, &schema).await
        }
    } else {
        PostgresSchedulerStore::connect(&url).await
    }
}
