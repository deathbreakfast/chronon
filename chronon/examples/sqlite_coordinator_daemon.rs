//! Same-host coordinator-only daemon backed by a shared SQLite file.
//!
//! ```bash
//! export CHRONON_SQLITE_PATH=/tmp/chronon-split.db
//! cargo run -p uf-chronon --example sqlite_coordinator_daemon --features sqlite
//! ```

use std::sync::Arc;

use chronon::prelude::*;
use chronon_backend_sqlite::SqliteSchedulerStore;
use chronon_core::JsonScriptContextFactory;
use chronon_executor::ScriptDescriptor;

fn noop_script(
    _ctx: Box<dyn ScriptContext>,
    _params: serde_json::Value,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = chronon::Result<()>> + Send>> {
    Box::pin(async { Ok(()) })
}

#[tokio::main]
async fn main() -> chronon::Result<()> {
    let path =
        std::env::var("CHRONON_SQLITE_PATH").unwrap_or_else(|_| "/tmp/chronon-split.db".into());
    let instance_id =
        std::env::var("CHRONON_INSTANCE_ID").unwrap_or_else(|_| "coordinator-0".into());

    let store: Arc<dyn SchedulerStore> = Arc::new(SqliteSchedulerStore::new(&path).await?);
    let registry = Arc::new({
        let mut r = ScriptRegistry::new();
        r.register(&ScriptDescriptor::new("daemon-noop", noop_script));
        r
    });

    let mut chronon = ChrononBuilder::new()
        .scheduler_store(store)
        .context_factory(Arc::new(JsonScriptContextFactory))
        .script_registry(registry)
        .instance_id(instance_id)
        .coordinator_only()
        .build()?;

    chronon.scheduler.init_partitions().await;
    eprintln!("sqlite_coordinator_daemon: running ({path})");
    chronon.run().await
}
