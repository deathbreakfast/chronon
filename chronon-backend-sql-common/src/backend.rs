use std::fmt;

use chronon_core::Result;
use sqlx::{Executor, Pool, Postgres, Sqlite};

use crate::error_map::map_err;
use crate::schema;

/// Max Postgres pool connections (`CHRONON_PG_POOL_SIZE`, default 5, cap 200).
#[must_use]
pub fn postgres_max_connections() -> u32 {
    std::env::var("CHRONON_PG_POOL_SIZE")
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(5)
        .clamp(1, 200)
}

/// `SQLite` uses `?` placeholders; `PostgreSQL` uses `$1`, `$2`, …
pub fn bind_sql(dialect: SqlDialect, sql: &str) -> String {
    match dialect {
        SqlDialect::Sqlite => sql.to_string(),
        SqlDialect::Postgres => {
            let mut out = String::with_capacity(sql.len());
            let mut n = 1u32;
            for ch in sql.chars() {
                if ch == '?' {
                    out.push('$');
                    out.push_str(&n.to_string());
                    n += 1;
                } else {
                    out.push(ch);
                }
            }
            out
        }
    }
}

/// SQL dialect for query variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqlDialect {
    /// `PostgreSQL`.
    Postgres,
    /// `SQLite`.
    Sqlite,
}

/// Connection pool for a SQL backend.
#[derive(Clone)]
pub enum SqlPool {
    /// `SQLite` pool.
    Sqlite(Pool<Sqlite>),
    /// `PostgreSQL` pool.
    Postgres(Pool<Postgres>),
}

impl fmt::Debug for SqlPool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sqlite(_) => f.debug_tuple("SqlPool::Sqlite").finish(),
            Self::Postgres(_) => f.debug_tuple("SqlPool::Postgres").finish(),
        }
    }
}

/// SQL-backed [`SchedulerStore`](chronon_core::store::SchedulerStore) (`PostgreSQL` or `SQLite`).
pub struct SqlSchedulerStore {
    pub(crate) pool: SqlPool,
    pub(crate) dialect: SqlDialect,
}

impl SqlSchedulerStore {
    /// Open a `SQLite` pool, bootstrap schema, and return a store.
    ///
    /// # Errors
    ///
    /// Returns a storage error if the pool connection or schema bootstrap fails.
    pub async fn connect_sqlite(url: &str) -> Result<Self> {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(5)
            .connect(url)
            .await
            .map_err(map_err)?;
        Self::from_sqlite_pool(pool).await
    }

    /// Open a `PostgreSQL` pool, bootstrap schema, and return a store.
    ///
    /// # Errors
    ///
    /// Returns a storage error if the pool connection or schema bootstrap fails.
    pub async fn connect_postgres(url: &str) -> Result<Self> {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(postgres_max_connections())
            .connect(url)
            .await
            .map_err(map_err)?;
        Self::from_postgres_pool(pool).await
    }

    /// Connect to `PostgreSQL` with an isolated schema for parallel tests.
    ///
    /// # Errors
    ///
    /// Returns a storage error if schema creation, pool connection, or bootstrap fails.
    pub async fn connect_postgres_isolated(url: &str, schema: &str) -> Result<Self> {
        let admin = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect(url)
            .await
            .map_err(map_err)?;
        let ddl = format!("CREATE SCHEMA IF NOT EXISTS \"{schema}\"");
        admin.execute(ddl.as_str()).await.map_err(map_err)?;
        drop(admin);

        let schema = schema.to_string();
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(postgres_max_connections())
            .after_connect(move |conn, _meta| {
                let schema = schema.clone();
                Box::pin(async move {
                    let sql = format!("SET search_path TO \"{schema}\"");
                    sqlx::query(&sql).execute(conn).await?;
                    Ok(())
                })
            })
            .connect(url)
            .await
            .map_err(map_err)?;
        Self::from_postgres_pool(pool).await
    }

    /// Attach to an existing isolated schema without re-running DDL bootstrap.
    ///
    /// Used when a test process already bootstrapped the schema and worker daemons join.
    ///
    /// # Errors
    ///
    /// Returns a storage error if the pool cannot be opened.
    pub async fn attach_postgres_isolated(url: &str, schema: &str) -> Result<Self> {
        let schema = schema.to_string();
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(postgres_max_connections())
            .after_connect(move |conn, _meta| {
                let schema = schema.clone();
                Box::pin(async move {
                    let sql = format!("SET search_path TO \"{schema}\"");
                    sqlx::query(&sql).execute(conn).await?;
                    Ok(())
                })
            })
            .connect(url)
            .await
            .map_err(map_err)?;
        Ok(Self {
            pool: SqlPool::Postgres(pool),
            dialect: SqlDialect::Postgres,
        })
    }

    /// Wrap an existing `SQLite` pool (schema bootstrap runs).
    ///
    /// # Errors
    ///
    /// Returns a storage error if schema bootstrap fails.
    pub async fn from_sqlite_pool(pool: Pool<Sqlite>) -> Result<Self> {
        let store = Self {
            pool: SqlPool::Sqlite(pool),
            dialect: SqlDialect::Sqlite,
        };
        schema::ensure_schema(&store).await?;
        Ok(store)
    }

    /// Wrap an existing `PostgreSQL` pool (schema bootstrap runs).
    ///
    /// # Errors
    ///
    /// Returns a storage error if schema bootstrap fails.
    pub async fn from_postgres_pool(pool: Pool<Postgres>) -> Result<Self> {
        let store = Self {
            pool: SqlPool::Postgres(pool),
            dialect: SqlDialect::Postgres,
        };
        schema::ensure_schema(&store).await?;
        Ok(store)
    }

    /// Underlying connection pool.
    #[must_use]
    pub const fn pool(&self) -> &SqlPool {
        &self.pool
    }

    /// Engine dialect.
    #[must_use]
    pub const fn dialect(&self) -> SqlDialect {
        self.dialect
    }

    pub(crate) async fn run_ddl(&self, ddl: &str) -> Result<()> {
        match &self.pool {
            SqlPool::Sqlite(pool) => {
                pool.execute(ddl).await.map_err(map_err)?;
            }
            SqlPool::Postgres(pool) => {
                pool.execute(ddl).await.map_err(map_err)?;
            }
        }
        Ok(())
    }

    /// Drop an isolated PostgreSQL schema (bench cell reset).
    ///
    /// # Errors
    ///
    /// Returns a storage error when the admin connection or DDL fails.
    pub async fn drop_postgres_schema(url: &str, schema: &str) -> Result<()> {
        let admin = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect(url)
            .await
            .map_err(map_err)?;
        let ddl = format!("DROP SCHEMA IF EXISTS \"{schema}\" CASCADE");
        admin.execute(ddl.as_str()).await.map_err(map_err)?;
        Ok(())
    }
}

impl fmt::Debug for SqlSchedulerStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SqlSchedulerStore")
            .field("dialect", &self.dialect)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::{bind_sql, SqlDialect};

    #[test]
    fn bind_sql_sqlite_passthrough() {
        let sql = "SELECT * FROM t WHERE id = ? AND name = ?";
        assert_eq!(bind_sql(SqlDialect::Sqlite, sql), sql);
    }

    #[test]
    fn bind_sql_postgres_renumbers_placeholders() {
        let sql = "UPDATE t SET a = ?, b = ? WHERE id = ?";
        assert_eq!(
            bind_sql(SqlDialect::Postgres, sql),
            "UPDATE t SET a = $1, b = $2 WHERE id = $3"
        );
    }
}
