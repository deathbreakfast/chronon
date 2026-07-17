//! Environment availability helpers for extended matrix rows.

use crate::matrix::StorageAdapter;

/// Whether a storage adapter can run live integration tests in the current environment.
pub fn extended_store_available(storage: StorageAdapter) -> bool {
    match storage {
        StorageAdapter::Mem | StorageAdapter::Sqlite => true,
        StorageAdapter::Postgres => std::env::var("CHRONON_POSTGRES_URL").is_ok(),
        StorageAdapter::PostgresRedis => {
            std::env::var("CHRONON_POSTGRES_URL").is_ok()
                && (std::env::var("CHRONON_REDIS_URL").is_ok()
                    || std::env::var("CHRONON_TEST_REDIS_URL").is_ok())
        }
    }
}

/// Human-readable skip reason when [`extended_store_available`] is false.
pub fn extended_store_skip_reason(storage: StorageAdapter) -> Option<&'static str> {
    if extended_store_available(storage) {
        return None;
    }
    Some(match storage {
        StorageAdapter::Mem | StorageAdapter::Sqlite => "always available",
        StorageAdapter::Postgres => "set CHRONON_POSTGRES_URL for postgres matrix",
        StorageAdapter::PostgresRedis => {
            "set CHRONON_POSTGRES_URL and CHRONON_REDIS_URL (or CHRONON_TEST_REDIS_URL) for postgres-redis matrix"
        }
    })
}
