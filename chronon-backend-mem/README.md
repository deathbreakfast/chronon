# chronon-backend-mem

In-memory `SchedulerStore` adapter (tests, CI, testkit default).

## Audience

**Integrators** wiring local evaluation and **bench/e2e** drivers.

## Compose

```rust
use std::sync::Arc;
use chronon_backend_mem::InMemorySchedulerStore;
use chronon_runtime::ChrononBuilder;

let store = Arc::new(InMemorySchedulerStore::new());
let chronon = ChrononBuilder::new()
    .scheduler_store(store)
    .embedded()
    .build()?;
```

## Bootstrap

```rust
use chronon_backend_mem::install_default_mem_store;
use chronon_core::default_store_from_global;

let _store = install_default_mem_store();
let resolved = default_store_from_global()?;
```

## Facade feature

Enable via `chronon` crate feature `mem`.

## Documentation

```bash
cargo doc -p chronon-backend-mem --no-deps --open
```
