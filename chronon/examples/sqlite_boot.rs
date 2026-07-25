//! Boot Chronon with a file-backed SQLite [`SchedulerStore`](chronon_core::SchedulerStore).
//!
//! Set `CHRONON_SQLITE_PATH` to a file path (default `/tmp/chronon-example.db`), or `:memory:`
//! for an ephemeral database.
//!
//! ```bash
//! cargo run -p uf-chronon --example sqlite_boot --features sqlite
//! CHRONON_SQLITE_PATH=:memory: cargo run -p uf-chronon --example sqlite_boot --features sqlite
//! ```

use std::sync::Arc;

use chronon::prelude::*;
use chronon_backend_sqlite::SqliteSchedulerStore;

#[tokio::main]
async fn main() -> chronon::Result<()> {
    let path =
        std::env::var("CHRONON_SQLITE_PATH").unwrap_or_else(|_| "/tmp/chronon-example.db".into());

    let store: Arc<dyn SchedulerStore> = if path == ":memory:" {
        Arc::new(SqliteSchedulerStore::connect("sqlite://:memory:").await?)
    } else {
        Arc::new(SqliteSchedulerStore::new(&path).await?)
    };

    let chronon = ChrononBuilder::new()
        .scheduler_store(store)
        .embedded()
        .build()?;

    assert_eq!(chronon.executor().script_count(), 0);
    eprintln!("Chronon booted with SQLite store ({path})");
    Ok(())
}
