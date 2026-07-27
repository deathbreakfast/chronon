# chronon-backend-postgres

Shared durable PostgreSQL [`SchedulerStore`](https://docs.rs/chronon-core/latest/chronon_core/trait.SchedulerStore.html) for production coordinator–worker clusters.

## Compose with Chronon

```rust
use std::sync::Arc;
use chronon::prelude::*;
use chronon_backend_postgres::PostgresSchedulerStore;

let store: Arc<dyn SchedulerStore> = Arc::new(
    PostgresSchedulerStore::connect("postgres://user:pass@localhost/chronon").await?,
);
let chronon = ChrononBuilder::new()
    .scheduler_store(store)
    .embedded()
    .build()?;
```

Runnable examples:

```bash
# Embedded boot
cargo run -p uf-chronon --example postgres_boot --features postgres

# Coordinator–worker split (shared CHRONON_POSTGRES_URL)
cargo run -p uf-chronon --example postgres_coordinator_daemon --features postgres &
CHRONON_INSTANCE_ID=worker-a cargo run -p uf-chronon --example postgres_worker_daemon --features postgres
```

Topology docs: [Embedded](https://docs.rs/uf-chronon/latest/chronon/index.html#embedded-one-process) /
[Coordinator–worker](https://docs.rs/uf-chronon/latest/chronon/index.html#coordinator-worker-split).
For production claim throughput, prefer Postgres + Redis (`coordinator_daemon` / `worker_daemon`).

## Environment

| Variable | Purpose |
|----------|---------|
| `CHRONON_POSTGRES_URL` | Primary URL for tests and CI |
| `CHRONON_TEST_POSTGRES_URL` | Fallback test URL |

Use `postgres_test_url()` to resolve URL precedence in test helpers.

Isolated schemas (`PostgresSchedulerStore::connect_isolated` / `CHRONON_POSTGRES_SCHEMA`)
must pass [`validate_postgres_schema_name`](https://docs.rs/chronon-backend-sql-common) —
only `^[A-Za-z_][A-Za-z0-9_]*$` up to 63 characters.

## Cargo feature

Enable via `chronon` crate feature `postgres`:

```toml
chronon = { git = "...", default-features = false, features = ["postgres"] }
```

## Contract tests

```bash
export CHRONON_POSTGRES_URL=postgres://user:pass@localhost/chronon
cargo test -p chronon-backend-postgres --tests -- --include-ignored
```

## Documentation

```bash
cargo doc -p chronon-backend-postgres --no-deps --open
```

See also: [`chronon-backend-sql-common`](../chronon-backend-sql-common/README.md).
