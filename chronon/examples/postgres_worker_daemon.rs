//! Worker daemon backed by PostgreSQL (no Redis overlay).
//!
//! ```bash
//! export CHRONON_POSTGRES_URL=postgres://user:pass@localhost/chronon
//! export CHRONON_INSTANCE_ID=worker-a
//! export CHRONON_WORKER_POOL=general
//! cargo run -p uf-chronon --example postgres_worker_daemon --features postgres
//! ```

use std::sync::Arc;

use chronon::prelude::*;
use chronon_backend_postgres::{postgres_store_from_env, postgres_test_url};
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
    let pg_url = postgres_test_url();
    let instance_id = std::env::var("CHRONON_INSTANCE_ID").unwrap_or_else(|_| "worker-0".into());
    let pool = std::env::var("CHRONON_WORKER_POOL").unwrap_or_else(|_| "general".into());

    let store: Arc<dyn SchedulerStore> = Arc::new(postgres_store_from_env().await?);
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

    eprintln!("postgres_worker_daemon: {instance_id} pool={pool} ({pg_url})");
    chronon.run().await
}
