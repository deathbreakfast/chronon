//! Binary entry point for the Chronon benchmark CLI.

use anyhow::Result;
use clap::Parser;

#[tokio::main]
async fn main() -> Result<()> {
    chronon_bench::cli::dispatch(chronon_bench::cli::Cli::parse()).await
}
