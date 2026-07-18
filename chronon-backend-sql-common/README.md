# chronon-backend-sql-common

Shared SQL [`SchedulerStore`](https://docs.rs/chronon-core/latest/chronon_core/trait.SchedulerStore.html) for PostgreSQL and SQLite.

## Audience

**Adapter authors** extending SQL persistence. Application integrators should use the thin wrappers:

- [`chronon-backend-postgres`](../chronon-backend-postgres/README.md)
- [`chronon-backend-sqlite`](../chronon-backend-sqlite/README.md)

## Stack position

```text
chronon-backend-{postgres,sqlite} → chronon-backend-sql-common → chronon-core
```

## Entry points

| Symbol | Purpose |
|--------|---------|
| [`SqlSchedulerStore`] | Connect, schema bootstrap, full port implementation |
| [`SqlDialect`] / [`SqlPool`] | Engine selection |
| [`bind_sql`] | Rewrite `?` placeholders to `$1`, … for Postgres |
| [`delegate_scheduler_store!`] | Macro for thin wrapper crates |

## When to depend directly

- Building a new SQL dialect wrapper (mirror postgres/sqlite crates)
- Contributing to the shared schema or query modules

Do **not** depend on this crate from application binaries — use the facade features (`sqlite`, `postgres`).

## Schema

Tables and indexes bootstrap on connect via [`ensure_schema`](https://docs.rs/chronon-backend-sql-common/latest/chronon_backend_sql_common/schema/fn.ensure_schema.html). No manual migration step in v0.1.1.

## Documentation

```bash
cargo doc -p chronon-backend-sql-common --no-deps --open
```

[`SqlSchedulerStore`]: https://docs.rs/chronon-backend-sql-common/latest/chronon_backend_sql_common/struct.SqlSchedulerStore.html
[`SqlDialect`]: https://docs.rs/chronon-backend-sql-common/latest/chronon_backend_sql_common/enum.SqlDialect.html
[`SqlPool`]: https://docs.rs/chronon-backend-sql-common/latest/chronon_backend_sql_common/enum.SqlPool.html
[`bind_sql`]: https://docs.rs/chronon-backend-sql-common/latest/chronon_backend_sql_common/fn.bind_sql.html
[`delegate_scheduler_store!`]: https://docs.rs/chronon-backend-sql-common/latest/chronon_backend_sql_common/macro.delegate_scheduler_store.html
