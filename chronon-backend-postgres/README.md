# chronon-backend-postgres

PostgreSQL [`SchedulerStore`](https://docs.rs/chronon-core/latest/chronon_core/trait.SchedulerStore.html) adapter for Chronon.

## Audience

**Backend engineers** deploying shared durable storage for production coordinator–worker clusters.

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

Runnable example: `cargo run -p uf-chronon --example postgres_boot --features postgres`.

## Environment

| Variable | Purpose |
|----------|---------|
| `CHRONON_POSTGRES_URL` | Primary URL for tests and CI |
| `CHRONON_TEST_POSTGRES_URL` | Fallback test URL |

Use `postgres_test_url()` to resolve URL precedence in test helpers.

## Facade feature

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
