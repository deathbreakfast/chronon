# chronon-core

Portable DTOs, `SchedulerStore` port, `StoreRouter`, errors, and identity ports — wire hosts via `StoreRouter`/`ChrononError`, implement persistence against portable DTOs, and expose the `ScriptContext` trait surface (macro details in `chronon-macros`).

## Role

- `Job`, `Run`, `JobRevision`, `Script`, schedule enums
- **`SchedulerStore`** — stable async trait for scheduler persistence
- **`StoreRouter`** — register named stores at host boot
- **`ScriptContext`**, **`ContextFactory`**, **`JsonScriptContextFactory`**, **`IdentityError`**
- **`ScriptHandle<P>`** — typed script name handle; `job` / `job_with_params` seed default [`Job`] values
- Coordinator-facing portable types

Third-party crates implement **`SchedulerStore`** only against DTOs exported here.

## Documentation

```bash
cargo doc -p chronon-core --no-deps --open
```
