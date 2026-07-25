//! Same-host worker daemon backed by a shared SQLite file.
//!
//! ```bash
//! export CHRONON_SQLITE_PATH=/tmp/chronon-split.db
//! export CHRONON_INSTANCE_ID=worker-a
//! export CHRONON_WORKER_POOL=general
//! cargo run -p uf-chronon --example sqlite_worker_daemon --features sqlite
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
    let instance_id = std::env::var("CHRONON_INSTANCE_ID").unwrap_or_else(|_| "worker-0".into());
    let pool = std::env::var("CHRONON_WORKER_POOL").unwrap_or_else(|_| "general".into());

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
        .instance_id(instance_id.clone())
        .worker(&pool)
        .build()?;

    eprintln!("sqlite_worker_daemon: {instance_id} pool={pool} ({path})");
    chronon.run().await
}
