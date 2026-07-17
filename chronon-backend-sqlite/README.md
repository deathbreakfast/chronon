# chronon-backend-sqlite

SQLite [`SchedulerStore`](https://docs.rs/chronon-core/latest/chronon_core/trait.SchedulerStore.html) adapter for Chronon.

## Audience

**Backend engineers** and **test authors** needing embedded, file-backed persistence.

**Single-writer limitation:** SQLite serializes writes. One Chronon scheduler + worker pool on a single host is fine; for concurrent claim-heavy workloads prefer PostgreSQL or the Postgres + Redis composite backend.

## Compose with Chronon

```rust
use std::sync::Arc;
use chronon::prelude::*;
use chronon_backend_sqlite::SqliteSchedulerStore;

let store: Arc<dyn SchedulerStore> = Arc::new(
    SqliteSchedulerStore::connect("sqlite://:memory:").await?,
);
let chronon = ChrononBuilder::new()
    .scheduler_store(store)
    .embedded()
    .build()?;
```

Runnable example: `cargo run -p uf-chronon --example sqlite_boot --features sqlite`.

## Configuration

| API | Use when |
|-----|----------|
| `SqliteSchedulerStore::new(path)` | File on disk (`/var/lib/chronon/chronon.db`) |
| `SqliteSchedulerStore::connect(url)` | Full URL including `:memory:` for tests |
| `SqliteSchedulerStore::from_pool(pool)` | Host already owns an `sqlx` pool |

Schema bootstrap runs automatically on connect.

## Facade feature

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
