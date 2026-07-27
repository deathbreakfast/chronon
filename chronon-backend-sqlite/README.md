# chronon-backend-sqlite

Embedded, file-backed SQLite [`SchedulerStore`](https://docs.rs/chronon-core/latest/chronon_core/trait.SchedulerStore.html) adapter for Chronon. SQLite serializes writes — one Chronon scheduler + worker pool on a single host is fine; for concurrent claim-heavy workloads prefer PostgreSQL or the Postgres + Redis composite backend.

## Compose with Chronon

```rust
use std::sync::Arc;
use chronon::prelude::*;
use chronon_backend_sqlite::SqliteSchedulerStore;

let store: Arc<dyn SchedulerStore> = Arc::new(
    SqliteSchedulerStore::new("/tmp/chronon-example.db").await?,
);
let chronon = ChrononBuilder::new()
    .scheduler_store(store)
    .embedded()
    .build()?;
```

Runnable examples (full runbook: [`chronon/README.md`](../chronon/README.md#how-to-run-examples)):

```bash
# Embedded (standalone)
cargo run -p uf-chronon --example sqlite_boot --features sqlite

# Coordinator–worker same-host — shared path; coordinator first; unique worker ids
export CHRONON_SQLITE_PATH=/tmp/chronon-split.db
# Terminal 1
cargo run -p uf-chronon --example sqlite_coordinator_daemon --features sqlite
# Terminal 2
CHRONON_INSTANCE_ID=worker-a cargo run -p uf-chronon --example sqlite_worker_daemon --features sqlite
# Terminal 3
CHRONON_INSTANCE_ID=worker-b cargo run -p uf-chronon --example sqlite_worker_daemon --features sqlite
```

Topology docs: [Embedded](https://docs.rs/uf-chronon/latest/chronon/index.html#embedded-one-process) /
[Coordinator–worker](https://docs.rs/uf-chronon/latest/chronon/index.html#coordinator-worker-split).

## Configuration

| API | Use when |
|-----|----------|
| `SqliteSchedulerStore::new(path)` | File on disk (`/var/lib/chronon/chronon.db`) |
| `SqliteSchedulerStore::connect(url)` | Full URL including `:memory:` for tests |
| `SqliteSchedulerStore::from_pool(pool)` | Host already owns an `sqlx` pool |
| `CHRONON_SQLITE_PATH` | Shared file path for examples / same-host split |

Schema bootstrap runs automatically on connect.

## Cargo feature

```toml
chronon = { git = "...", default-features = false, features = ["sqlite"] }
```

## Contract tests

```bash
cargo test -p chronon-backend-sqlite --tests
```

Runs in PR CI alongside `chronon-backend-mem`.

## Documentation

```bash
cargo doc -p chronon-backend-sqlite --no-deps --open
```

See also: [`chronon-backend-sql-common`](../chronon-backend-sql-common/README.md).
