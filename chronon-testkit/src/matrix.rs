//! Benchmark and e2e matrix dimensions (storage, deployment, topology, telemetry).

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

/// Storage backend adapter under test.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum StorageAdapter {
    /// In-memory store (default CI slice).
    #[default]
    Mem,
    /// File-backed SQLite store (PR CI durable slice).
    Sqlite,
    /// PostgreSQL store (extended CI on tag builds).
    Postgres,
    /// PostgreSQL authority + Redis claim queue (extended CI on tag builds).
    PostgresRedis,
}

impl StorageAdapter {
    /// Stable kebab-case slug for reports and CLI flags.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mem => "mem",
            Self::Sqlite => "sqlite",
            Self::Postgres => "postgres",
            Self::PostgresRedis => "postgres-redis",
        }
    }

    /// Parse a CLI or config slug into a variant.
    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "mem" => Ok(Self::Mem),
            "sqlite" => Ok(Self::Sqlite),
            "postgres" => Ok(Self::Postgres),
            "postgres-redis" => Ok(Self::PostgresRedis),
            other => bail!("unknown storage adapter: {other}"),
        }
    }
}

/// Runtime deployment shape under test.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum DeploymentKind {
    /// Single-process scheduler + worker loops.
    #[default]
    Embedded,
    /// Separate in-process coordinator and worker tasks.
    CoordinatorWorker,
    /// Client against a remote coordinator HTTP base URL.
    RemoteClient,
}

impl DeploymentKind {
    /// Stable kebab-case slug for reports and CLI flags.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Embedded => "embedded",
            Self::CoordinatorWorker => "coordinator-worker",
            Self::RemoteClient => "remote-client",
        }
    }

    /// Parse a CLI or config slug into a variant.
    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "embedded" => Ok(Self::Embedded),
            "coordinator-worker" => Ok(Self::CoordinatorWorker),
            "remote-client" => Ok(Self::RemoteClient),
            other => bail!("unknown deployment shape: {other}"),
        }
    }
}

/// Physical or logical topology label for a matrix row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Topology {
    /// Minimal isolated lab (default bench/e2e slice).
    #[default]
    IsolatedLab,
    /// Monolith with embedded scheduler and worker.
    MonolithEmbedded,
    /// Split coordinator server and worker processes.
    SplitChrononServer,
    /// Worker talking to a remote coordinator.
    RemoteCoordinator,
}

impl Topology {
    /// Stable kebab-case slug for reports and CLI flags.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::IsolatedLab => "isolated-lab",
            Self::MonolithEmbedded => "monolith-embedded",
            Self::SplitChrononServer => "split-chronon-server",
            Self::RemoteCoordinator => "remote-coordinator",
        }
    }

    /// Parse a CLI or config slug into a variant.
    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "isolated-lab" => Ok(Self::IsolatedLab),
            "monolith-embedded" => Ok(Self::MonolithEmbedded),
            "split-chronon-server" => Ok(Self::SplitChrononServer),
            "remote-coordinator" => Ok(Self::RemoteCoordinator),
            other => bail!("unknown topology: {other}"),
        }
    }
}

/// Telemetry sink adapter under test.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum TelemetryAdapter {
    /// Recording sink (assertions) or discard, depending on scenario mode.
    #[default]
    Off,
    /// Stderr console sink for debugging.
    Console,
}

impl TelemetryAdapter {
    /// Stable kebab-case slug for reports and CLI flags.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Console => "console",
        }
    }

    /// Parse a CLI or config slug into a variant.
    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "off" => Ok(Self::Off),
            "console" => Ok(Self::Console),
            other => bail!("unknown telemetry adapter: {other}"),
        }
    }
}

/// One row in the storage × deployment × topology × telemetry benchmark matrix.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct MatrixSpec {
    /// Storage backend dimension.
    pub storage: StorageAdapter,
    /// Deployment shape dimension.
    pub deployment: DeploymentKind,
    /// Topology label dimension.
    pub topology: Topology,
    /// Telemetry sink dimension.
    pub telemetry: TelemetryAdapter,
}

impl MatrixSpec {
    /// Default CI slice: mem storage, embedded deployment, isolated lab, telemetry off.
    pub fn ci_mem_embedded() -> Self {
        Self::default()
    }

    /// CI slice with split coordinator + worker deployment.
    pub fn ci_mem_coordinator_worker() -> Self {
        Self {
            deployment: DeploymentKind::CoordinatorWorker,
            ..Self::default()
        }
    }

    /// Concatenated slug used in bench report filenames.
    pub fn report_slug(&self) -> String {
        format!(
            "{}-{}-{}-{}",
            self.storage.as_str(),
            self.deployment.as_str(),
            self.topology.as_str(),
            self.telemetry.as_str(),
        )
    }
}
