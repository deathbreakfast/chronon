//! Long-running coordinator-only daemon for distributed E2E on AWS.
//!
//! ```bash
//! export CHRONON_POSTGRES_URL=postgres://user:pass@host:5432/chronon
//! export CHRONON_REDIS_URL=redis://host:6379
//! cargo run -p uf-chronon --example coordinator_daemon --features postgres,redis
//! ```

use std::sync::Arc;

use chronon::prelude::*;
use chronon_backend_postgres::{postgres_store_from_env, postgres_test_url};
use chronon_backend_redis::{PostgresRedisSchedulerStore, RedisQueueLayer};
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
    let redis_url = std::env::var("CHRONON_REDIS_URL")
        .or_else(|_| std::env::var("CHRONON_TEST_REDIS_URL"))
        .unwrap_or_else(|_| "redis://127.0.0.1:6379".into());
    let instance_id =
        std::env::var("CHRONON_INSTANCE_ID").unwrap_or_else(|_| "coordinator-0".into());

    let sql: Arc<dyn SchedulerStore> = Arc::new(postgres_store_from_env().await?);
    let redis_prefix = std::env::var("CHRONON_REDIS_PREFIX").ok();
    let redis = RedisQueueLayer::connect(&redis_url, redis_prefix.as_deref()).await?;
    let store: Arc<dyn SchedulerStore> = Arc::new(PostgresRedisSchedulerStore::new(sql, redis));

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
    eeprintln!("coordinator_daemon: running ({pg_url}, {redis_url})");
    chronon.run().await
}
