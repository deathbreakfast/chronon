//! Chronon is a Rust cron and run-once scheduler for services: typed script handlers,
//! durable job/run history, and an optional coordinator–worker split behind a thin
//! [`SchedulerStore`](chronon_core::SchedulerStore) port.
//!
//! Wire storage once with [`ChrononBuilder`], register scripts with [`script`], schedule
//! [`Job`](chronon_core::Job)s, then call [`Chronon::run`]. Swap `mem`, `sqlite`, Postgres,
//! or Postgres+Redis without changing script code.
//!
//! ## Features
//!
//! - **Typed scripts** — `#[chronon::script]` registers handlers with inventory; params stay typed.
//! - **Durable jobs and runs** — schedule config, revisions, and execution history on
//!   [`SchedulerStore`](chronon_core::SchedulerStore).
//! - **Composable storage** — in-memory, SQLite, PostgreSQL, or Postgres + Redis claim overlay.
//! - **Embedded or split topology** — one process, or coordinator / worker / remote HTTP client
//!   (see [Choose a topology](#choose-a-topology)).
//! - **Host identity** — [`ContextFactory`](chronon_core::ContextFactory) rebuilds run-time
//!   context from stored JSON.
//! - **Optional HTTP API** — mount [`chronon_router`] (`axum` feature).
//!
//! *Cron and run-once scheduling without locking you into one database or a full workflow engine.*
//!
//! This crate ships with **no default features** (`default = []`). Enable explicitly:
//! `mem`, `sqlite`, `postgres`, `redis` (requires `postgres`), `axum`, `telemetry-console`.
//!
//! # Getting started
//!
//! You always define scripts with `#[chronon::script]` and schedule via the generated
//! [`ScriptHandle`] (`nightly_cleanup().job_with_params(...)`, then
//! [`CoordinatorService::upsert_job`] / [`CoordinatorService::run_now`]). What changes is
//! **which process ticks the schedule and which process executes scripts**.
//!
//! ## Choose a topology
//!
//! - **[Mode 1 — Embedded](#mode-1--embedded-one-binary)** — one binary schedules **and**
//!   executes. Start here.
//! - **[Mode 2 — Coordinator + worker](#mode-2--coordinator--worker-two-binaries)** — one
//!   process ticks / enqueues; one or more **worker** binaries claim and run scripts.
//! - **[Mode 3 — Remote HTTP client](#mode-3--remote-http-client-optional)** — your app has
//!   **no** local Chronon loops; it talks to a coordinator HTTP API via
//!   [`RemoteCoordinatorClient`]. Optional.
//!
//! | Topology | Builder | Store fit | When to use |
//! |----------|---------|-----------|-------------|
//! | Embedded | [`.embedded()`](ChrononBuilder::embedded) | mem / sqlite / postgres / postgres+redis | Local, single host, or simple production |
//! | Coordinator | [`.coordinator_only()`](ChrononBuilder::coordinator_only) | Shared durable (postgres ± redis) | Scale-out: tick only |
//! | Worker | [`.worker(pool)`](ChrononBuilder::worker) | Same shared store | Scale-out: claim + execute |
//! | Remote client | [`.remote_coordinator(url)`](ChrononBuilder::remote_coordinator) | None locally | Schedule via HTTP (Mode 3) |
//!
//! Topology is [`DeploymentShape`] on [`ChrononBuilder`]. After you pick a mode, continue with
//! [define a script](#4-define-a-script) (shared by every mode).
//!
//! ## Mode 1 — Embedded (one binary)
//!
//! This process runs the scheduler tick **and** the worker. There is no second binary.
//!
//! ```text
//! Your app ──ScriptHandle / upsert_job──► Chronon ──tick + claim──► script handlers
//!                                            │
//!                                            └──► mem / SQLite / Postgres / Postgres+Redis
//! ```
//!
//! | Backend | Type | Feature | Topology | Mode 1 boot |
//! |---------|------|---------|----------|-------------|
//! | In-memory | [`InMemorySchedulerStore`] | `mem` | embedded only | Below |
//! | SQLite | [`SqliteSchedulerStore`] | `sqlite` | embedded | [sqlite crate](../chronon_backend_sqlite/index.html#mode-1--embedded) |
//! | PostgreSQL | [`PostgresSchedulerStore`] | `postgres` | embedded or Mode 2 | [postgres crate](../chronon_backend_postgres/index.html#mode-1--embedded) |
//! | Postgres + Redis | [`PostgresRedisSchedulerStore`] | `postgres,redis` | embedded or Mode 2 | [redis crate](../chronon_backend_redis/index.html#mode-1--embedded) |
//!
//! **In-memory first run** — `#[chronon::script]` generates a handle factory and
//! `NightlyCleanupParams`; prefer that over stringly `Job::new`:
//!
//! ```ignore
//! use std::sync::Arc;
//! use chronon::prelude::*;
//! use chronon::InMemorySchedulerStore;
//!
//! #[chronon::script(name = "nightly_cleanup")]
//! async fn nightly_cleanup(
//!     ctx: Box<dyn ScriptContext>,
//!     retention_days: u32,
//! ) -> chronon::Result<()> {
//!     let _ = (ctx.label(), retention_days);
//!     Ok(())
//! }
//!
//! # async fn main() -> chronon::Result<()> {
//! let chronon = ChrononBuilder::new()
//!     .scheduler_store(Arc::new(InMemorySchedulerStore::new()))
//!     .context_factory(Arc::new(JsonScriptContextFactory))
//!     .embedded()
//!     .auto_registry()
//!     .build()?;
//!
//! let mut job = nightly_cleanup().job_with_params(
//!     "nightly-schedule",
//!     &NightlyCleanupParams { retention_days: 7 },
//! )?;
//! job.schedule_kind = ScheduleKind::Cron;
//! job.cron_expr = Some("0 2 * * *".into());
//! job.timezone = Some("UTC".into());
//! chronon.coordinator_service().upsert_job(job).await?;
//! // chronon.scheduler.init_partitions().await;
//! // chronon.run().await?;
//! # Ok(())
//! # }
//! ```
//!
//! Runnable: `script_handle_job`, `script_macro`, `embedded_tick`, `run_now` (`--features mem`).
//! Other stores: follow the Mode 1 links in the table above. Then continue with
//! [define a script](#4-define-a-script).
//!
//! ## Mode 2 — Coordinator + worker (two binaries)
//!
//! Use this when you want **scale-out execution** or to keep scheduling separate from script
//! work. Both processes share the same durable store; they do **not** share memory.
//! [`InMemorySchedulerStore`] cannot cross process boundaries — Mode 2 needs SQLite
//! (same-host file), Postgres, or Postgres+Redis.
//!
//! ```text
//! Coordinator binary ──tick──► shared store ──claim──► Worker binary(ies)
//!        │                                              │
//!        └── ScriptHandle / upsert_job           script handlers
//! ```
//!
//! ### What you create
//!
//! | Piece | Purpose |
//! |-------|---------|
//! | Shared scripts | Same `#[chronon::script]` names linked into **workers** |
//! | Coordinator binary | [`.coordinator_only()`](ChrononBuilder::coordinator_only) — tick + partitions; **no** worker slots |
//! | Worker binary(ies) | [`.worker(pool)`](ChrononBuilder::worker) — claim + execute; unique [`.instance_id()`](ChrononBuilder::instance_id) |
//! | Shared store | Postgres (add Redis for production claim throughput) |
//!
//! ### Pick a shared store
//!
//! Wire coordinator and worker from the adapter pages (production default: Postgres + Redis):
//!
//! | Backend | Feature | Mode 2 coordinator | Mode 2 worker |
//! |---------|---------|--------------------|---------------|
//! | Postgres + Redis | `postgres,redis` | [Coordinator](../chronon_backend_redis/index.html#mode-2--coordinator-binary) | [Worker](../chronon_backend_redis/index.html#mode-2--worker-binary) |
//! | PostgreSQL | `postgres` | [Coordinator](../chronon_backend_postgres/index.html#mode-2--coordinator-binary) | [Worker](../chronon_backend_postgres/index.html#mode-2--worker-binary) |
//! | SQLite (same host) | `sqlite` | [Coordinator](../chronon_backend_sqlite/index.html#mode-2--coordinator-binary) | [Worker](../chronon_backend_sqlite/index.html#mode-2--worker-binary) |
//!
//! ### Run both
//!
//! 1. Start Postgres (and Redis). Set `CHRONON_POSTGRES_URL` / `CHRONON_REDIS_URL`.
//! 2. Start the **coordinator** (`init_partitions` then [`Chronon::run`]).
//! 3. Start one or more **workers** with unique `CHRONON_INSTANCE_ID` values.
//! 4. Upsert jobs (via [`ScriptHandle`]) from the coordinator, an Axum host, or
//!    [Mode 3](#mode-3--remote-http-client-optional).
//!
//! ```bash
//! export CHRONON_POSTGRES_URL=postgres://user:pass@localhost/chronon
//! export CHRONON_REDIS_URL=redis://127.0.0.1:6379
//! cargo run -p uf-chronon --example coordinator_daemon --features postgres,redis &
//! CHRONON_INSTANCE_ID=worker-a cargo run -p uf-chronon --example worker_daemon --features postgres,redis
//! ```
//!
//! ## Mode 3 — Remote HTTP client (optional)
//!
//! Use this when an application process should **schedule or trigger jobs** but must not run
//! Chronon loops locally. Pair it with a host that mounts [`chronon_router`] on a Mode 1 or
//! Mode 2 coordinator process.
//!
//! ```text
//! App binary ──RemoteCoordinatorClient──HTTP──► API host (chronon_router)
//!                                                    │
//!                                                    └── Mode 1 or Mode 2 coordinator + store
//! ```
//!
//! **API host** — nest the router under [`API_PREFIX`] (`/api/chronon`). Sketch:
//! `axum_host` (`mem,axum`).
//!
//! **App binary** — build a [`Job`](chronon_core::Job) from your [`ScriptHandle`], then call
//! [`RemoteCoordinatorClient`] (do not call [`Chronon::run`]):
//!
//! ```ignore
//! use chronon::prelude::*;
//!
//! let base = resolve_remote_base_url()
//!     .unwrap_or_else(|| "http://127.0.0.1:8080".into());
//! let client = RemoteCoordinatorClient::new(base);
//!
//! let mut job = nightly_cleanup().job_with_params(
//!     "nightly-schedule",
//!     &NightlyCleanupParams { retention_days: 7 },
//! )?;
//! job.schedule_kind = ScheduleKind::Manual;
//! client.upsert_job(job.clone()).await?;
//! let _run_id = client.run_now(&job.job_id).await?;
//! ```
//!
//! Set `CHRONON_REMOTE_BASE_URL` for [`resolve_remote_base_url`]. Timeout:
//! `CHRONON_REMOTE_HTTP_TIMEOUT_MS` (default 3000).
//!
//! ## 4. Define a script
//!
//! `#[chronon::script]` registers the handler **and** turns the function into a
//! [`ScriptHandle`] factory. Parameter types become a generated `*Params` struct
//! (for example `NightlyCleanupParams`).
//!
//! ```ignore
//! use chronon::prelude::*;
//!
//! #[chronon::script(name = "nightly_cleanup")]
//! async fn nightly_cleanup(
//!     ctx: Box<dyn ScriptContext>,
//!     retention_days: u32,
//! ) -> chronon::Result<()> {
//!     println!("{}: retaining {retention_days} days", ctx.label());
//!     Ok(())
//! }
//!
//! // nightly_cleanup() -> ScriptHandle<NightlyCleanupParams>
//! // NightlyCleanupParams { retention_days: u32 }
//! ```
//!
//! Use [`.auto_registry()`](ChrononBuilder::auto_registry) so inventory picks up every
//! `#[chronon::script]` linked into the binary. In Mode 2, scripts must be linked into
//! **worker** binaries (that is where they run).
//!
//! See [`script`], [`ScriptHandle`], and [`ScriptContext`](chronon_core::ScriptContext).
//! Runnable: `script_handle_job`, `script_macro`.
//!
//! ## 5. Schedule and trigger jobs
//!
//! Prefer the generated handle over stringly `Job::new("…", "script_name")`. Set
//! [`ScheduleKind`](chronon_core::ScheduleKind) on the returned [`Job`](chronon_core::Job):
//!
//! | [`ScheduleKind`](chronon_core::ScheduleKind) | Behavior |
//! |----------------------------------------------|----------|
//! | `Cron` | Recurring; set `cron_expr` (+ optional `timezone`) |
//! | `RunOnce` | Fires when `next_run_at` is due |
//! | `Manual` | Never due for tick — only [`CoordinatorService::run_now`] |
//!
//! ```ignore
//! use chronon::prelude::*;
//!
//! let mut nightly = nightly_cleanup().job_with_params(
//!     "nightly-schedule",
//!     &NightlyCleanupParams { retention_days: 7 },
//! )?;
//! nightly.schedule_kind = ScheduleKind::Cron;
//! nightly.cron_expr = Some("0 2 * * *".into());
//! chronon.coordinator_service().upsert_job(nightly).await?;
//!
//! let mut manual = nightly_cleanup().job_with_params(
//!     "cleanup-now",
//!     &NightlyCleanupParams { retention_days: 30 },
//! )?;
//! manual.schedule_kind = ScheduleKind::Manual;
//! let id = manual.job_id.clone();
//! chronon.coordinator_service().upsert_job(manual).await?;
//! chronon.coordinator_service().run_now(&id).await?;
//! ```
//!
//! Cron uses standard five-field syntax (optional sixth field for seconds). Parse helpers:
//! [`CronExpr`]. Runnable: `script_handle_job`, `run_now`, `embedded_tick`.
//!
//! Storage wiring: [Mode 1](#mode-1--embedded-one-binary) (mem below; other backends on adapter
//! crates) and [Mode 2](#mode-2--coordinator--worker-two-binaries) (link table).
//!
//! # Notes
//!
//! - **No default Cargo features** — enable `mem`, `sqlite`, `postgres`, `redis`, and/or `axum`
//!   explicitly. Document the facade with `--all-features` so rustdoc links resolve.
//! - **Mode 2 scripts live on workers** — inventory must be linked into the binary that calls
//!   `.worker(...)`; the coordinator ticks but does not execute handlers.
//! - **Call `scheduler.init_partitions().await` before [`Chronon::run`]** on embedded and
//!   coordinator-only shapes.
//! - **RemoteClient must not call [`Chronon::run`]** — that shape returns an error; use
//!   [`RemoteCoordinatorClient`].
//! - **`mem` is Mode 1 only** — it does not cross process boundaries.
//!
//! # Architecture
//!
//! Your application owns identity policy and business logic. Chronon owns scheduling semantics:
//! due queries, claiming, cron evaluation, and script dispatch.
//!
//! ```text
//! Your app / worker binary
//!         │
//!         ▼
//!  ChrononBuilder ──► SchedulerStore port ──► mem | sqlite | postgres | postgres+redis | custom
//!         │
//!         ├──► Scheduler (tick / partitions)
//!         └──► Executor + ScriptRegistry  ◄── ContextFactory / #[chronon::script]
//! ```
//!
//! Mode 2 splits the loops across processes that share the store:
//!
//! ```text
//! Coordinator ──.coordinator_only()──► tick + partitions ──► SchedulerStore
//! Worker(s)   ──.worker(pool)────────► claim + execute   ──► same SchedulerStore
//! ```
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
//! | Worker pool | `.worker(pool)` / env | `CHRONON_WORKER_POOL` | `"general"` |
//! | Worker concurrency | — | `CHRONON_WORKER_CONCURRENCY` | 4 |
//! | Remote base URL | `.remote_coordinator(url)` | `CHRONON_REMOTE_BASE_URL` | — |
//!
//! Lease TTLs and tick batch limits are environment-only. See `chronon-scheduler` crate
//! documentation for the full table.
//!
//! # Cargo features
//!
//! | Feature | Type | Status |
//! |---------|------|--------|
//! | `mem` | [`InMemorySchedulerStore`] | Ready — tests and local Mode 1 |
//! | `sqlite` | [`SqliteSchedulerStore`] | Ready — embedded file-backed |
//! | `postgres` | [`PostgresSchedulerStore`] | Ready — shared durable |
//! | `redis` | [`PostgresRedisSchedulerStore`] | Ready — Postgres + Redis claim overlay (**requires `postgres`**) |
//! | `axum` | [`chronon_router`], HTTP DTOs | Ready — mount on host Axum server |
//! | `telemetry-console` | Documents `ConsoleSink` usage | Optional marker (`ConsoleSink` always re-exported) |
//!
//! # Runnable examples
//!
//! | Example | Topology | Features |
//! |---------|----------|----------|
//! | `script_macro` | Mode 1 | `mem` |
//! | `script_handle_job` | Mode 1 | `mem` |
//! | `run_now` | Mode 1 | `mem` |
//! | `embedded_tick` | Mode 1 | `mem` |
//! | `store_router_boot` | Mode 1 | `mem` |
//! | `sqlite_boot` | Mode 1 | `sqlite` |
//! | `postgres_boot` | Mode 1 | `postgres` |
//! | `postgres_redis_boot` | Mode 1 | `postgres,redis` |
//! | `axum_host` | Mode 1 + HTTP | `mem,axum` |
//! | `coordinator_daemon` | Mode 2 coordinator | `postgres,redis` |
//! | `worker_daemon` | Mode 2 worker | `postgres,redis` |
//!
//! ```bash
//! cargo run -p uf-chronon --example script_handle_job --features mem
//! cargo run -p uf-chronon --example coordinator_daemon --features postgres,redis
//! cargo run -p uf-chronon --example worker_daemon --features postgres,redis
//! ```

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
pub use chronon_runtime::{
    builder, resolve_remote_base_url, Chronon, ChrononBuilder, CoordinatorService, DeploymentShape,
    RemoteCoordinatorClient,
};
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
