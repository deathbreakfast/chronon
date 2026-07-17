//! Chronon — cron and run-once scheduling for Rust services.
//!
//! Chronon provides typed script handlers, durable job/run history, and optional
//! coordinator–worker split behind a thin [`SchedulerStore`](chronon_core::SchedulerStore) port.
//! Feature-gated backends and HTTP adapters sit behind the public facade.
//!
//! # Getting started
//!
//! Cargo examples are **not** auto-indexed by rustdoc. Use the table below (also in
//! [`chronon/README.md`](https://github.com/unified-field-dev/chronon/blob/main/chronon/README.md)),
//! or `cargo run -p uf-chronon --example <name> --features …`.
//!
//! | Example | Shows | Features |
//! |---------|-------|----------|
//! | `script_macro` | `#[chronon::script]` + `Job::new` + `upsert_job` + tick | `mem` |
//! | `script_handle_job` | Macro [`ScriptHandle`] → default job + typed params | `mem` |
//! | `run_now` | Manual job + [`CoordinatorService::run_now`](CoordinatorService::run_now) | `mem` |
//! | `embedded_tick` | Due job enqueue via `tick_once` | `mem` |
//! | `store_router_boot` | Global [`StoreRouter`](chronon_core::StoreRouter) install | `mem` |
//! | `sqlite_boot` | SQLite store boot | `sqlite` |
//! | `postgres_boot` / `postgres_redis_boot` | Durable backends | `postgres` / `postgres,redis` |
//! | `axum_host` | Mount HTTP router | `mem,axum` |
//! | `coordinator_daemon` / `worker_daemon` | Split deployment shapes | `postgres,redis` |
//!
//! # Documentation map
//!
//! Full snippets live on the linked items (not repeated here).
//!
//! - **Boot the runtime** — [`ChrononBuilder`], [`DeploymentShape`], [`Chronon::run`]
//! - **Define scripts** — [`script`] attribute, [`ScriptContext`](chronon_core::ScriptContext),
//!   [`ContextFactory`](chronon_core::ContextFactory)
//! - **Schedule a job** — [`Job`](chronon_core::Job), [`CoordinatorService::upsert_job`](CoordinatorService),
//!   typed defaults via [`ScriptHandle`]
//! - **Run immediately** — [`CoordinatorService::run_now`](CoordinatorService::run_now) (manual / on-demand)
//! - **Persist jobs and runs** — [`SchedulerStore`](chronon_core::SchedulerStore),
//!   [`Run`](chronon_core::Run)
//! - **Manage jobs programmatically** — [`CoordinatorService`]
//! - **Parse cron** — [`CronExpr`]
//! - **Register storage** — `install_default_mem_store` (`mem` feature), [`StoreRouter`](chronon_core::StoreRouter)
//! - **HTTP API** — `chronon_router` (`axum` feature)
//! - **Metrics** — [`TelemetrySink`]
//!
//! # Configuration
//!
//! Settings merge in this order: explicit [`ChrononBuilder`] values override environment
//! defaults where both exist.
//!
//! | Setting | Builder API | Environment | Default |
//! |---------|-------------|-------------|---------|
//! | Store | `.scheduler_store()` / `.scheduler_store_from_global()` | — | required |
//! | Context factory | `.context_factory()` | — | `NoOpContextFactory` |
//! | Telemetry | `.telemetry_sink()` | — | `NoOpSink` |
//! | Script registry | `.script_registry()` / `.auto_registry()` | — | empty or inventory |
//! | Tick interval | `.tick_interval_ms()` | `CHRONON_TICK_INTERVAL_MS` | 250 ms |
//! | Instance id | `.instance_id()` | — | random UUID |
//! | Partition count | — (env only) | `CHRONON_NUM_PARTITIONS` | 64 |
//!
//! ### Backend connection (pass to store constructors, not `ChrononBuilder`)
//!
//! | Backend | Configuration |
//! |---------|---------------|
//! | PostgreSQL | URL to `PostgresSchedulerStore::connect`; `CHRONON_POSTGRES_URL` / `CHRONON_TEST_POSTGRES_URL` for tests |
//! | SQLite | Path or URL to `SqliteSchedulerStore` |
//! | Redis overlay | URL to `RedisQueueLayer::connect`; optional key prefix (default `chronon`); `CHRONON_REDIS_URL` in production |
//!
//! Lease TTLs, tick batch limits, worker pool, and worker concurrency are environment-only.
//! See `chronon-scheduler` crate documentation for the full environment variable table.
//!
//! # Cargo features
//!
//! No features are enabled by default. Enable explicitly:
//!
//! | Feature | Type | Status |
//! |---------|------|--------|
//! | `mem` | `InMemorySchedulerStore` | Ready — tests and local dev |
//! | `sqlite` | `SqliteSchedulerStore` | Ready — embedded file-backed |
//! | `postgres` | `PostgresSchedulerStore` | Ready — shared durable |
//! | `redis` | `PostgresRedisSchedulerStore` | Ready — Postgres + Redis claim overlay (**requires `postgres` feature**) |
//! | `axum` | `chronon_router`, HTTP DTOs | Ready — mount on host Axum server |
//! | `telemetry-console` | Documents `ConsoleSink` usage | Optional marker (`ConsoleSink` always re-exported) |

pub use chronon_macros::script;
pub use quark::inventory;

pub mod prelude {
    //! Curated re-exports for **application developers** building Chronon worker binaries.
    //!
    //! One-import surface for models, runtime boot, scheduler, executor, and the [`script`] macro.
    //! Prefer `use chronon::prelude::*;` in worker binaries and integration tests rather than
    //! importing internal crates directly. For durable storage wiring, also enable facade features
    //! (`sqlite`, `postgres`, `redis`) and construct the matching [`SchedulerStore`] adapter.

    pub use chronon_core::{
        ContextFactory, Job, JobRevision, JsonScriptContextFactory, NoOpContextFactory,
        NoOpScriptContext, Run, RunStatus, ScheduleKind, SchedulerStore, Script, ScriptContext,
        ScriptHandle, StoreRouter, ChrononError, Result, DEFAULT_STORE_NAME,
    };
    pub use chronon_executor::{Executor, ExecutorEvent, ScriptDescriptor, ScriptRegistry};
    pub use chronon_runtime::{
        builder, Chronon, ChrononBuilder, CoordinatorService, DeploymentShape, JobSummary,
        RemoteCoordinatorClient, resolve_remote_base_url,
    };
    pub use chronon_scheduler::{CronExpr, Scheduler, SchedulerConfig};
    pub use crate::script;
}

pub use chronon_core as core;
pub use chronon_runtime::{builder, Chronon, ChrononBuilder, CoordinatorService, DeploymentShape};
pub use chronon_core::{ChrononError, Result, ScriptHandle};
pub use chronon_executor::{ScriptDescriptor, ScriptRegistry};
pub use chronon_scheduler::CronExpr;

#[cfg(feature = "axum")]
pub use chronon_axum::{chronon_router, ApiResponse, ChrononState, API_PREFIX};

#[cfg(feature = "mem")]
pub use chronon_backend_mem::{install_default_mem_store, InMemorySchedulerStore};

#[cfg(feature = "sqlite")]
pub use chronon_backend_sqlite::SqliteSchedulerStore;

#[cfg(feature = "postgres")]
pub use chronon_backend_postgres::{postgres_test_url, PostgresSchedulerStore};

#[cfg(feature = "redis")]
pub use chronon_backend_redis::{PostgresRedisSchedulerStore, RedisQueueLayer};

pub use chronon_telemetry::{ConsoleSink, NoOpSink, TelemetrySink};
