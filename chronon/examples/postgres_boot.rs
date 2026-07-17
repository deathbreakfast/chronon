//! Boot Chronon with a PostgreSQL-backed [`SchedulerStore`](chronon_core::SchedulerStore).
//!
//! Requires a running Postgres instance. Set `CHRONON_POSTGRES_URL` or pass a URL below.
//!
//! ```bash
//! export CHRONON_POSTGRES_URL=postgres://user:pass@localhost/chronon
//! cargo run -p uf-chronon --example postgres_boot --features postgres
//! ```

use std::sync::Arc;

use chronon::prelude::*;
use chronon_backend_postgres::{postgres_test_url, PostgresSchedulerStore};

#[tokio::main]
async fn main() -> chronon::Result<()> {
    let url = postgres_test_url();
    let store: Arc<dyn SchedulerStore> =
        Arc::new(PostgresSchedulerStore::connect(&url).await?);
    let chronon = ChrononBuilder::new()
        .scheduler_store(store)
        .embedded()
        .build()?;

    assert_eq!(chronon.executor().script_count(), 0);
    println!("Chronon booted with PostgreSQL store ({url})");
    Ok(())
}
