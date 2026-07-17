//! Shared postgres/redis store for multibench BM-CH7 fleet cells.

use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use chronon_backend_postgres::PostgresSchedulerStore;
use chronon_backend_redis::{PostgresRedisSchedulerStore, RedisQueueLayer};
use chronon_core::store::SchedulerStore;

use crate::matrix::StorageAdapter;

use super::BootstrapSession;

const ATTACH_POLL_MS: u64 = 25;
const DEFAULT_ATTACH_WAIT_SECS: f64 = 120.0;

/// Sanitize a bench cell tag into a postgres schema / redis prefix token.
#[must_use]
pub fn bench_cell_namespace(cell_id: &str) -> String {
    let mut out = String::with_capacity(cell_id.len());
    for c in cell_id.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else {
            out.push('_');
        }
    }
    out.trim_matches('_').to_string()
}

async fn postgres_cell_ready(store: &PostgresSchedulerStore) -> bool {
    store.list_jobs().await.is_ok()
        && store.list_runs_filtered(None, None, 0, 1).await.is_ok()
}

async fn attach_postgres_with_wait(url: &str, schema: &str, timeout_secs: f64) -> Result<PostgresSchedulerStore> {
    let deadline = Instant::now() + Duration::from_secs_f64(timeout_secs);
    loop {
        let ready = match PostgresSchedulerStore::attach_isolated(url, schema).await {
            Ok(store) if postgres_cell_ready(&store).await => Some(store),
            Ok(_) | Err(_) => None,
        };
        if let Some(store) = ready {
            return Ok(store);
        }
        if Instant::now() >= deadline {
            anyhow::bail!("timed out waiting for multibench cell bootstrap on schema {schema}");
        }
        tokio::time::sleep(Duration::from_millis(ATTACH_POLL_MS)).await;
    }
}

fn attach_wait_secs() -> f64 {
    std::env::var("CHRONON_CH7_PREFILL_WAIT_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_ATTACH_WAIT_SECS)
}

async fn drop_bench_schema(url: &str, schema: &str) -> Result<()> {
    PostgresSchedulerStore::drop_isolated_schema(url, schema).await?;
    Ok(())
}

impl BootstrapSession {
    /// Install a store shared by all multibench clients in one cell (`cell_id`).
    ///
    /// Client 0 bootstraps schema/redis; drain-only clients attach after leader DDL.
    ///
    /// # Errors
    ///
    /// Returns an error when storage is unsupported or backend connect fails.
    pub async fn install_multibench_cell(&mut self, cell_id: &str, is_leader: bool) -> Result<()> {
        self.env_guard = Some(super::env_guard::EnvGuard::set(
            "CHRONON_NUM_PARTITIONS",
            &self.num_partitions.to_string(),
        ));

        let namespace = bench_cell_namespace(cell_id);
        let schema = format!("chronon_bench_{namespace}");
        let redis_prefix = format!("chronon_bench_{namespace}");

        let store: Arc<dyn SchedulerStore> = match self.matrix.storage {
            StorageAdapter::Postgres => {
                let url = chronon_backend_postgres::postgres_test_url();
                if is_leader {
                    drop_bench_schema(&url, &schema).await?;
                }
                let backend = if is_leader {
                    Arc::new(PostgresSchedulerStore::connect_isolated(&url, &schema).await?)
                } else {
                    Arc::new(
                        attach_postgres_with_wait(&url, &schema, attach_wait_secs()).await?,
                    )
                };
                self.postgres_schema = Some(schema);
                backend
            }
            StorageAdapter::PostgresRedis => {
                let url = chronon_backend_postgres::postgres_test_url();
                if is_leader {
                    drop_bench_schema(&url, &schema).await?;
                }
                let sql = if is_leader {
                    Arc::new(PostgresSchedulerStore::connect_isolated(&url, &schema).await?)
                } else {
                    Arc::new(
                        attach_postgres_with_wait(&url, &schema, attach_wait_secs()).await?,
                    )
                };
                let redis_url = std::env::var("CHRONON_REDIS_URL")
                    .or_else(|_| std::env::var("CHRONON_TEST_REDIS_URL"))
                    .map_err(|_| {
                        anyhow::anyhow!(
                            "PostgresRedis matrix requires CHRONON_REDIS_URL or CHRONON_TEST_REDIS_URL"
                        )
                    })?;
                let redis = RedisQueueLayer::connect(&redis_url, Some(&redis_prefix)).await?;
                if is_leader {
                    redis.flush_keys().await?;
                }
                self.postgres_schema = Some(schema);
                Arc::new(PostgresRedisSchedulerStore::new(sql, redis))
            }
            StorageAdapter::Mem | StorageAdapter::Sqlite => {
                return self.install().await;
            }
        };

        self.store = Some(store);
        self.ready = true;
        Ok(())
    }
}
