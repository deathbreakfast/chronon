//! Synthetic Chronon benchmark harness (BM-CH* / BM-CHL*).
//!
//! **Audience:** performance engineers running scheduler, store, and deployment-shape
//! campaigns documented in [`EXPERIMENTS.md`](../EXPERIMENTS.md).
//!
//! # Entry points
//!
//! - CLI binary `chronon-bench` — `run`, `matrix`, `scaling-curve` subcommands
//! - [`config::BenchRunConfig`] — per-experiment sweep defaults
//! - [`experiments::subset_experiments`] — matrix slice registry
//! - [`report::BenchReport`] — JSON report shape
//!
//! # Examples
//!
//! ```no_run
//! use chronon_bench::config::BenchRunConfig;
//!
//! let cfg = BenchRunConfig::for_experiment("bm-ch7");
//! assert_eq!(cfg.worker_count, 32);
//! ```

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::print_stdout,
    clippy::unused_async
)]

pub mod cli;
pub mod config;
pub mod experiments;
pub mod matrix;
pub mod pass_eval;
pub mod projection;
pub mod report;
pub mod runners;
pub mod stats;
