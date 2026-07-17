//! Boot Chronon with a SQLite-backed [`SchedulerStore`](chronon_core::SchedulerStore).
//!
//! ```bash
//! cargo run -p uf-chronon --example sqlite_boot --features sqlite
//! ```

use std::sync::Arc;

use chronon::prelude::*;
use chronon_backend_sqlite::SqliteSchedulerStore;

#[tokio::main]
async fn main() -> chronon::Result<()> {
    let store: Arc<dyn SchedulerStore> =
        Arc::new(SqliteSchedulerStore::connect("sqlite://:memory:").await?);
    let chronon = ChrononBuilder::new()
        .scheduler_store(store)
        .embedded()
        .build()?;

    assert_eq!(chronon.executor().script_count(), 0);
    println!("Chronon booted with SQLite store");
    Ok(())
}
