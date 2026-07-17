# chronon-macros

Proc macros for Chronon scheduled scripts.

Provides `#[chronon::script]` for defining scripts with typed parameter structs and Quark inventory registration.

**Audience:** Application developers authoring scheduled script handlers.

## Quick start

1. Annotate an async function with `#[chronon::script(name = "...")]`.
2. First parameter must be `Box<dyn ScriptContext>`.
3. Add crate dependencies (below).
4. Link the defining crate into your worker binary.
5. Boot Chronon with `.auto_registry()` and a `ContextFactory`.
6. Enqueue jobs via the scheduler store / HTTP API using the script name.

Runnable examples:
- [`chronon/examples/script_macro.rs`](../chronon/examples/script_macro.rs) — stringly `Job::new` + upsert
- [`chronon/examples/script_handle_job.rs`](../chronon/examples/script_handle_job.rs) — typed `ScriptHandle` defaults

## Consumer dependencies

Crates that define scripts need:

```toml
chronon-macros = { version = "0.1" }
quark = { package = "uf-quark", version = "0.1.1" }
chronon-executor = { version = "0.1" }
chronon-core = { version = "0.1" }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

Or depend on the facade with the macro re-exported (crates.io package is **`uf-chronon`**; the unrelated crate named `chronon` is not this project):

```toml
chronon = { package = "uf-chronon", version = "0.1", features = ["mem"] }
```

## Link closure

Inventory registration happens at link time. The binary that runs Chronon workers must depend on every crate that defines `#[chronon::script]` handlers.

## Identity

Handlers receive `Box<dyn ScriptContext>`. At boot, install a factory on `ChrononBuilder`:

- `JsonScriptContextFactory` for examples and actor-json-only handlers.
- A custom `ContextFactory` when handlers need application-specific session state rebuilt from `actor_json`.

Host applications often publish identity adapters as separate crates and recover typed context inside the handler body.

## Documentation

```bash
cargo doc -p chronon-macros --no-deps --open
```
