//! Chronon core — portable types and the [`SchedulerStore`] persistence port.
//!
//! This crate defines domain models, error types, script identity ports, and the async storage
//! trait that backends implement. It has no runtime, scheduler, or HTTP dependencies.
//!
//! # Goals
//!
//! - Stable DTOs ([`Job`], [`Run`], [`Script`]) shared by scheduler, executor, and adapters
//! - A single [`SchedulerStore`] port for jobs, runs, leases, and coordinator metadata
//! - Host-injectable script identity via [`ScriptContext`] and [`ContextFactory`]
//!
//! # Non-goals
//!
//! - Running tick loops or executing scripts (see `chronon-runtime` and `chronon-executor`)
//! - Choosing a storage backend (hosts register implementations at boot)
//!
//! # Modules
//!
//! - [`store`] — [`SchedulerStore`] trait and persistence contract
//! - [`models`] — jobs, runs, revisions, workers, schedule enums
//! - [`context`] — [`ScriptContext`] and [`ContextFactory`] for script dispatch
//! - [`router`] — named store registration via [`StoreRouter`]
//! - [`error`] — [`ChrononError`] and [`Result`]
//!
//! # Getting started
//!
//! Implement [`SchedulerStore`] for your storage substrate, then register it on
//! [`StoreRouter::register_global`] or pass the store directly to `ChrononBuilder` in `chronon-runtime`.
//!
//! See also: [`DEFAULT_STORE_NAME`], [`ScriptHandle`].

pub mod context;
pub mod error;
pub mod handle;
pub mod models;
pub mod router;
pub mod store;

#[cfg(test)]
mod unit_tests;

pub use context::{
    ContextFactory, IdentityError, JsonScriptContextFactory, NoOpContextFactory, NoOpScriptContext,
    ScriptContext,
};
pub use error::{ChrononError, Result};
pub use handle::ScriptHandle;
pub use models::{
    Job, JobRevision, Lease, MisfirePolicy, PartitionAssignment, RetryPolicy, Run, RunStatus,
    ScheduleKind, SchedulerLeader, Script, Worker, WorkerStatus,
};
pub use router::{default_store_from_global, StoreRouter, DEFAULT_STORE_NAME};
pub use store::SchedulerStore;
