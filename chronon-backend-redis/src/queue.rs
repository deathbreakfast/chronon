//! Redis sorted-set ready queue for run claims.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use chronon_core::error::ChrononError;
use chronon_core::Result;
use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use tokio::sync::Mutex;

/// Redis ZSET layer for queued runs (`{prefix}:ready:{pool}` or hash-tagged for cluster).
///
/// Used only as the claim hot path inside [`crate::PostgresRedisSchedulerStore`] — not a
/// standalone [`SchedulerStore`](chronon_core::SchedulerStore). Connect with [`Self::connect`]:
///
/// | Knob | Effect |
/// |------|--------|
/// | `url` / `CHRONON_REDIS_URL` | Standalone Redis |
/// | `CHRONON_REDIS_CLUSTER_URLS` | Comma-separated cluster nodes |
/// | `CHRONON_REDIS_HASH_TAGS=1` | Hash-tagged pool keys for slot affinity |
/// | `key_prefix` (default `chronon`) | Key namespace |
///
/// # Examples
///
/// ```no_run
/// use chronon_backend_redis::RedisQueueLayer;
///
/// # async fn demo() -> chronon_core::Result<()> {
/// let redis = RedisQueueLayer::connect("redis://127.0.0.1:6379", Some("myapp")).await?;
/// # let _ = redis;
/// # Ok(())
/// # }
/// ```
pub struct RedisQueueLayer {
    single: Option<ConnectionManager>,
    cluster: Option<Arc<Mutex<redis::cluster_async::ClusterConnection>>>,
    key_prefix: String,
    hash_tags: bool,
}

impl std::fmt::Debug for RedisQueueLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RedisQueueLayer")
            .field("key_prefix", &self.key_prefix)
            .field("hash_tags", &self.hash_tags)
            .field("cluster", &self.cluster.is_some())
            .finish_non_exhaustive()
    }
}

impl RedisQueueLayer {
    /// Connect to Redis at `url` with optional key prefix (default `chronon`).
    ///
    /// When `CHRONON_REDIS_CLUSTER_URLS` is set (comma-separated), uses Redis Cluster.
    /// When `CHRONON_REDIS_HASH_TAGS=1`, pool keys use hash tags for cluster slot affinity.
    ///
    /// # Errors
    ///
    /// Returns a storage error when the connection cannot be established.
    pub async fn connect(url: &str, key_prefix: Option<&str>) -> Result<Self> {
        let hash_tags = std::env::var("CHRONON_REDIS_HASH_TAGS")
            .is_ok_and(|v| matches!(v.as_str(), "1" | "true" | "yes"));
        let prefix = key_prefix.unwrap_or("chronon").to_string();

        if let Ok(urls) = std::env::var("CHRONON_REDIS_CLUSTER_URLS") {
            let nodes: Vec<String> = urls
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(String::from)
                .collect();
            if !nodes.is_empty() {
                let client = redis::cluster::ClusterClient::new(nodes).map_err(map_err)?;
                let conn = client.get_async_connection().await.map_err(map_err)?;
                return Ok(Self {
                    single: None,
                    cluster: Some(Arc::new(Mutex::new(conn))),
                    key_prefix: prefix,
                    hash_tags: true,
                });
            }
        }

        let client = redis::Client::open(url).map_err(map_err)?;
        let conn = ConnectionManager::new(client).await.map_err(map_err)?;
        Ok(Self {
            single: Some(conn),
            cluster: None,
            key_prefix: prefix,
            hash_tags,
        })
    }

    /// Redis URL for tests (`CHRONON_TEST_REDIS_URL`, then `CHRONON_REDIS_URL`, or local default).
    #[must_use]
    pub fn test_url() -> String {
        std::env::var("CHRONON_TEST_REDIS_URL")
            .or_else(|_| std::env::var("CHRONON_REDIS_URL"))
            .unwrap_or_else(|_| "redis://127.0.0.1:6379".into())
    }

    fn ready_key(&self, pool_id: &str) -> String {
        format_ready_key(&self.key_prefix, pool_id, self.hash_tags)
    }

    /// Enqueue a run id ordered by `scheduled_for` (ZADD score = epoch millis).
    ///
    /// # Errors
    ///
    /// Returns a storage error when Redis commands fail.
    pub async fn enqueue_run(
        &self,
        pool_id: &str,
        run_id: &str,
        scheduled_for: DateTime<Utc>,
    ) -> Result<()> {
        let score = scheduled_for.timestamp_millis() as f64;
        let key = self.ready_key(pool_id);
        if let Some(conn) = &self.single {
            let mut conn = conn.clone();
            let _: () = conn.zadd(key, run_id, score).await.map_err(map_err)?;
        } else if let Some(conn) = &self.cluster {
            let mut conn = conn.lock().await;
            let _: () = conn.zadd(key, run_id, score).await.map_err(map_err)?;
        }
        Ok(())
    }

    /// Atomically pop the earliest **due** run id (`score <= now`) from the pool queue.
    ///
    /// # Errors
    ///
    /// Returns a storage error when Redis commands fail.
    pub async fn claim_next_run_id(
        &self,
        pool_id: &str,
        now: DateTime<Utc>,
    ) -> Result<Option<String>> {
        let ids = self.claim_next_run_ids(pool_id, 1, now).await?;
        Ok(ids.into_iter().next())
    }

    /// Pop up to `count` earliest due run ids (`scheduled_for` score ≤ `now`).
    ///
    /// # Errors
    ///
    /// Returns a storage error when Redis commands fail.
    pub async fn claim_next_run_ids(
        &self,
        pool_id: &str,
        count: usize,
        now: DateTime<Utc>,
    ) -> Result<Vec<String>> {
        if count == 0 {
            return Ok(Vec::new());
        }
        let key = self.ready_key(pool_id);
        let max_score = now.timestamp_millis() as f64;
        // Atomic: take due members then remove them from the ZSET.
        let script = redis::Script::new(
            r"
            local ids = redis.call('ZRANGEBYSCORE', KEYS[1], '-inf', ARGV[1], 'LIMIT', 0, tonumber(ARGV[2]))
            for _, id in ipairs(ids) do
              redis.call('ZREM', KEYS[1], id)
            end
            return ids
            ",
        );
        let ids: Vec<String> = if let Some(conn) = &self.single {
            let mut conn = conn.clone();
            script
                .key(&key)
                .arg(max_score)
                .arg(count)
                .invoke_async(&mut conn)
                .await
                .map_err(map_err)?
        } else if let Some(conn) = &self.cluster {
            let mut conn = conn.lock().await;
            script
                .key(&key)
                .arg(max_score)
                .arg(count)
                .invoke_async(&mut *conn)
                .await
                .map_err(map_err)?
        } else {
            Vec::new()
        };
        Ok(ids)
    }

    /// Remove a run from the ready queue (e.g. after cancellation).
    ///
    /// # Errors
    ///
    /// Returns a storage error when Redis commands fail.
    pub async fn remove_run(&self, pool_id: &str, run_id: &str) -> Result<()> {
        let key = self.ready_key(pool_id);
        if let Some(conn) = &self.single {
            let mut conn = conn.clone();
            let _: () = conn.zrem(key, run_id).await.map_err(map_err)?;
        } else if let Some(conn) = &self.cluster {
            let mut conn = conn.lock().await;
            let _: () = conn.zrem(key, run_id).await.map_err(map_err)?;
        }
        Ok(())
    }

    /// Delete all keys with this layer's prefix (test isolation).
    ///
    /// # Errors
    ///
    /// Returns a storage error when Redis commands fail.
    pub async fn flush_keys(&self) -> Result<()> {
        if self.cluster.is_some() {
            return Ok(());
        }
        let Some(single) = &self.single else {
            return Ok(());
        };
        let pattern = format!("{}:*", self.key_prefix);
        let mut conn = single.clone();
        let mut cursor = 0_u64;
        loop {
            let (next, batch): (u64, Vec<String>) = redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg(&pattern)
                .arg("COUNT")
                .arg(500)
                .query_async(&mut conn)
                .await
                .map_err(map_err)?;
            if !batch.is_empty() {
                let _: () = conn.del(batch).await.map_err(map_err)?;
            }
            cursor = next;
            if cursor == 0 {
                break;
            }
        }
        Ok(())
    }
}

fn format_ready_key(prefix: &str, pool_id: &str, hash_tags: bool) -> String {
    if hash_tags {
        format!("{prefix}:{{{pool_id}}}:ready")
    } else {
        format!("{prefix}:ready:{pool_id}")
    }
}

fn map_err(e: impl std::fmt::Display) -> ChrononError {
    ChrononError::StorageError(e.to_string())
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::{format_ready_key, RedisQueueLayer};

    #[test]
    fn ready_key_plain_and_hash_tagged() {
        assert_eq!(
            format_ready_key("chronon_test", "workers", false),
            "chronon_test:ready:workers"
        );
        assert_eq!(
            format_ready_key("chronon_test", "general-0", true),
            "chronon_test:{general-0}:ready"
        );
    }

    async fn layer() -> Option<RedisQueueLayer> {
        let url = RedisQueueLayer::test_url();
        let prefix = format!("chronon_test_{}", uuid_like());
        let connect = RedisQueueLayer::connect(&url, Some(&prefix));
        let layer = tokio::time::timeout(std::time::Duration::from_secs(2), connect)
            .await
            .ok()?
            .ok()?;
        tokio::time::timeout(std::time::Duration::from_secs(2), layer.flush_keys())
            .await
            .ok()?
            .ok()?;
        Some(layer)
    }

    fn uuid_like() -> String {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or_else(|_| "0".into(), |d| d.as_nanos().to_string())
    }

    #[tokio::test]
    async fn enqueue_and_claim_orders_by_scheduled_for() {
        let Some(layer) = layer().await else {
            return;
        };
        let pool = "workers-order";
        let now = Utc::now();
        layer
            .enqueue_run(pool, "run-late", now)
            .await
            .expect("enqueue");
        layer
            .enqueue_run(pool, "run-early", now - chrono::Duration::minutes(1))
            .await
            .expect("enqueue");

        let first = layer.claim_next_run_id(pool, now).await.expect("claim");
        assert_eq!(first.as_deref(), Some("run-early"));

        let second = layer.claim_next_run_id(pool, now).await.expect("claim");
        assert_eq!(second.as_deref(), Some("run-late"));

        let empty = layer.claim_next_run_id(pool, now).await.expect("claim");
        assert!(empty.is_none());
    }

    #[tokio::test]
    async fn claim_skips_future_scheduled_for() {
        let Some(layer) = layer().await else {
            return;
        };
        let pool = "workers-future";
        let now = Utc::now();
        layer
            .enqueue_run(pool, "run-future", now + chrono::Duration::minutes(5))
            .await
            .expect("enqueue");
        let empty = layer.claim_next_run_id(pool, now).await.expect("claim");
        assert!(empty.is_none());
    }

    #[tokio::test]
    async fn claim_next_run_ids_batch() {
        let Some(layer) = layer().await else {
            return;
        };
        let pool = "workers-batch";
        let now = Utc::now();
        for i in 0..3_u64 {
            layer
                .enqueue_run(pool, &format!("run-{i}"), now)
                .await
                .expect("enqueue");
        }
        let batch = layer
            .claim_next_run_ids(pool, 2, now)
            .await
            .expect("batch");
        assert_eq!(batch.len(), 2);
    }
}
