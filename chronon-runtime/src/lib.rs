//! Chronon runtime assembly and deployment loops.
//!
//! Wires store, scheduler, executor, and background loops via [`ChrononBuilder`]. Hosts select
//! a [`DeploymentShape`] (embedded, coordinator-only, worker, or remote client) at build time.
//!
//! # Documentation map
//!
//! - **Configure and build** — [`ChrononBuilder`], [`DeploymentShape`]
//! - **Run until shutdown** — [`Chronon::run`], [`Chronon::shutdown`]
//! - **Job and run API** — [`CoordinatorService`]
//! - **Remote coordinator** — [`RemoteCoordinatorClient`], [`resolve_remote_base_url`]
//!
//! # Configuration
//!
//! [`ChrononBuilder::tick_interval_ms`] overrides `CHRONON_TICK_INTERVAL_MS`. Partition count
//! is read from `CHRONON_NUM_PARTITIONS` only (not configurable on the builder). See the
//! `chronon-scheduler` crate for the full environment variable reference.
//!
//! See also: [`builder`] (alias for [`ChrononBuilder::new`]).

mod builder;
mod coordinator;
mod coordinator_service;
mod embedded;
mod env;
mod events;
mod remote_client;
mod retry;
mod runtime;
mod worker;

pub use builder::{builder, ChrononBuilder, DeploymentShape};
pub use coordinator_service::CoordinatorService;
pub use remote_client::{resolve_remote_base_url, JobSummary, RemoteCoordinatorClient};
pub use runtime::Chronon;
