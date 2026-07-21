//! [`BootstrapSession`] install and Chronon build helpers.

use std::sync::Arc;

use anyhow::Result;
use chronon_backend_mem::InMemorySchedulerStore;
use chronon_backend_postgres::PostgresSchedulerStore;
use chronon_backend_redis::{PostgresRedisSchedulerStore, RedisQueueLayer};
use chronon_backend_sqlite::SqliteSchedulerStore;
use chronon_core::store::SchedulerStore;
use chronon_core::JsonScriptContextFactory;
use chronon_runtime::{Chronon, ChrononBuilder};

use super::{fresh_registry, telemetry_for_matrix, BootstrapSession};
use crate::matrix::{DeploymentKind, StorageAdapter};

impl BootstrapSession {
    /// Install storage and telemetry for the matrix row.
    ///
    /// # Errors
    ///
    /// Returns an error when the storage backend cannot be opened or configured.
    pub async fn install(&mut self) -> Result<()> {
        self.env_guard = Some(super::env_guard::EnvGuard::set(
            "CHRONON_NUM_PARTITIONS",
            &self.num_partitions.to_string(),
        ));

        let store: Arc<dyn SchedulerStore> = match self.matrix.storage {
            StorageAdapter::Mem => Arc::new(InMemorySchedulerStore::new()),
            StorageAdapter::Sqlite => {
                let temp = tempfile::tempdir()?;
                let path = temp.path().join("chronon.db");
                let backend = Arc::new(SqliteSchedulerStore::new(&path).await?);
                self.sqlite_temp = Some(temp);
                backend
            }
            StorageAdapter::Postgres => {
                let url = chronon_backend_postgres::postgres_test_url();
                let schema = format!("chronon_test_{}", uuid::Uuid::new_v4());
                let backend =
                    Arc::new(PostgresSchedulerStore::connect_isolated(&url, &schema).await?);
                self.postgres_schema = Some(schema);
                backend
            }
            StorageAdapter::PostgresRedis => {
                let url = chronon_backend_postgres::postgres_test_url();
                let schema = format!("chronon_test_{}", uuid::Uuid::new_v4());
                let sql = Arc::new(PostgresSchedulerStore::connect_isolated(&url, &schema).await?);
                let redis_url = std::env::var("CHRONON_REDIS_URL")
                    .or_else(|_| std::env::var("CHRONON_TEST_REDIS_URL"))
                    .map_err(|_| {
                        anyhow::anyhow!(
                            "PostgresRedis matrix requires CHRONON_REDIS_URL or CHRONON_TEST_REDIS_URL"
                        )
                    })?;
                let prefix = format!("chronon_test_{}", uuid::Uuid::new_v4());
                let redis = RedisQueueLayer::connect(&redis_url, Some(&prefix)).await?;
                redis.flush_keys().await?;
                self.postgres_schema = Some(schema);
                Arc::new(PostgresRedisSchedulerStore::new(sql, redis))
            }
        };

        self.store = Some(store);
        self.ready = true;
        Ok(())
    }

    /// Return a built [`Chronon`], constructing one via [`BootstrapSession::build_chronon`] if needed.
    pub fn ensure_chronon(&mut self) -> Result<&Chronon> {
        if self.chronon.is_none() {
            self.build_chronon()?;
        }
        Ok(self.chronon.as_ref().expect("chronon built"))
    }

    /// Build a [`Chronon`] for the current matrix row without starting background loops.
    pub fn build_chronon(&mut self) -> Result<&Chronon> {
        let store = self
            .store
            .clone()
            .ok_or_else(|| anyhow::anyhow!("BootstrapSession::install must run first"))?;
        let telemetry = telemetry_for_matrix(&self.matrix, &self.telemetry);
        let registry = fresh_registry();

        let builder = ChrononBuilder::new()
            .scheduler_store(store)
            .context_factory(Arc::new(JsonScriptContextFactory))
            .telemetry_sink(telemetry)
            .script_registry(registry);

        let chronon = match self.matrix.deployment {
            DeploymentKind::Embedded => builder.embedded().build(),
            DeploymentKind::CoordinatorWorker => builder.coordinator_only().build(),
            DeploymentKind::RemoteClient => builder
                .remote_coordinator(
                    chronon_runtime::resolve_remote_base_url()
                        .unwrap_or_else(|| "http://127.0.0.1:3000".to_string()),
                )
                .build(),
        }
        .map_err(|e| anyhow::anyhow!("build chronon: {e}"))?;
        self.chronon = Some(chronon);
        Ok(self.chronon.as_ref().expect("chronon"))
    }

    /// Initialize partition ownership on the scheduler store.
    pub async fn init_partitions(&mut self) -> Result<()> {
        let chronon = self.ensure_chronon()?;
        chronon.scheduler.init_partitions().await;
        Ok(())
    }

    /// Execute one coordinator tick and return enqueue statistics.
    pub async fn tick_once(&mut self) -> Result<chronon_scheduler::TickResult> {
        Ok(self.ensure_chronon()?.tick_once().await?)
    }

    /// Stop embedded or split background loops started by this session.
    pub async fn shutdown_embedded(&mut self) -> Result<()> {
        if let Some(handle) = self.embedded.take() {
            handle.shutdown().await;
        }
        if let Some(handle) = self.split.take() {
            handle.shutdown().await;
        }
        self.chronon = None;
        Ok(())
    }

    /// Store as a trait object for scenario steps and fixtures.
    pub fn store_dyn(&self) -> Result<Arc<dyn SchedulerStore>> {
        self.store
            .clone()
            .ok_or_else(|| anyhow::anyhow!("install first"))
    }
}
