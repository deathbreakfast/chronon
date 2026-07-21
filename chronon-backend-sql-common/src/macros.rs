//! Internal SQL query helpers for dialect-aware pool dispatch.
//!
//! **Audience:** internal — used by [`SqlSchedulerStore`](crate::SqlSchedulerStore) modules.

/// Execute a parameterized SQL statement on the store pool.
#[macro_export]
macro_rules! sql_execute {
    ($store:expr, $sql:expr, |$q:ident| $body:expr) => {{
        match &$store.pool {
            $crate::SqlPool::Sqlite(pool) => {
                let $q = sqlx::query($sql);
                let $q = $body;
                $q.execute(pool).await.map_err($crate::error_map::map_err)?;
            }
            $crate::SqlPool::Postgres(pool) => {
                let $q = sqlx::query($sql);
                let $q = $body;
                $q.execute(pool).await.map_err($crate::error_map::map_err)?;
            }
        }
        Ok::<(), chronon_core::error::ChrononError>(())
    }};
}

/// Fetch zero or one row and map it through a closure.
#[macro_export]
macro_rules! sql_fetch_optional_map {
    ($store:expr, $sql:expr, |$q:ident| $bind:expr, |$row:ident| $map:expr) => {{
        match &$store.pool {
            $crate::SqlPool::Sqlite(pool) => {
                let $q = sqlx::query($sql);
                let $q = $bind;
                match $q
                    .fetch_optional(pool)
                    .await
                    .map_err($crate::error_map::map_err)?
                {
                    Some($row) => Ok(Some($map?)),
                    None => Ok(None),
                }
            }
            $crate::SqlPool::Postgres(pool) => {
                let $q = sqlx::query($sql);
                let $q = $bind;
                match $q
                    .fetch_optional(pool)
                    .await
                    .map_err($crate::error_map::map_err)?
                {
                    Some($row) => Ok(Some($map?)),
                    None => Ok(None),
                }
            }
        }
    }};
}

/// Fetch exactly one row and map it through a closure.
#[macro_export]
macro_rules! sql_fetch_one_map {
    ($store:expr, $sql:expr, |$q:ident| $bind:expr, |$row:ident| $map:expr) => {{
        match &$store.pool {
            $crate::SqlPool::Sqlite(pool) => {
                let $q = sqlx::query($sql);
                let $q = $bind;
                let $row = $q
                    .fetch_one(pool)
                    .await
                    .map_err($crate::error_map::map_err)?;
                $map
            }
            $crate::SqlPool::Postgres(pool) => {
                let $q = sqlx::query($sql);
                let $q = $bind;
                let $row = $q
                    .fetch_one(pool)
                    .await
                    .map_err($crate::error_map::map_err)?;
                $map
            }
        }
    }};
}

/// Fetch all rows and map each through a closure.
#[macro_export]
macro_rules! sql_fetch_all_map {
    ($store:expr, $sql:expr, |$q:ident| $bind:expr, |$row:ident| $map:expr) => {{
        match &$store.pool {
            $crate::SqlPool::Sqlite(pool) => {
                let $q = sqlx::query($sql);
                let $q = $bind;
                let rows = $q
                    .fetch_all(pool)
                    .await
                    .map_err($crate::error_map::map_err)?;
                rows.iter()
                    .map(|$row| $map)
                    .collect::<chronon_core::Result<Vec<_>>>()
            }
            $crate::SqlPool::Postgres(pool) => {
                let $q = sqlx::query($sql);
                let $q = $bind;
                let rows = $q
                    .fetch_all(pool)
                    .await
                    .map_err($crate::error_map::map_err)?;
                rows.iter()
                    .map(|$row| $map)
                    .collect::<chronon_core::Result<Vec<_>>>()
            }
        }
    }};
}
