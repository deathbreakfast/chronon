//! Shared happy/sad correctness catalog for matrix E2E expansion.
//!
//! Adapter authors append storage backends to [`e2e_storage_backends`] and matrix
//! macros expand the suite automatically.
//!
//! When adding a scenario, update [`invoke_catalog_scenario_ids!`] and [`mem_catalog_entries!`] below.

use crate::matrix::{MatrixSpec, StorageAdapter};
use crate::scenario::ScenarioSpec;
use crate::shared_store::extended_store_available;
use crate::{BootstrapSession, RunMode, ScenarioRunner};

/// Forwards the canonical scenario id list to `$m` (used by matrix suite macros).
#[macro_export]
macro_rules! invoke_catalog_scenario_ids {
    ($m:path) => {
        $m!(
            scheduler_tick_smoke,
            due_job_enqueue,
            script_run_success,
            run_once_idempotent,
            telemetry_lifecycle,
            partition_due_filter,
            not_due_no_enqueue,
            pause_resume_smoke,
            script_run_failure,
            run_now_smoke,
            job_revisions_smoke,
            counting_exactly_once,
            wait_run_timeout,
        );
    };
}

/// Happy vs sad path label for catalog entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathKind {
    /// Expected success / policy-compliant behavior.
    Happy,
    /// Expected rejection, failure, or error path.
    Sad,
}

/// Deployment slice for a catalog row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogDeployment {
    /// Single-process embedded scheduler + worker.
    Embedded,
    /// In-process coordinator + worker split.
    CoordinatorWorker,
}

impl CatalogDeployment {
    fn matrix_spec(self, storage: StorageAdapter) -> MatrixSpec {
        MatrixSpec {
            storage,
            deployment: match self {
                Self::Embedded => crate::matrix::DeploymentKind::Embedded,
                Self::CoordinatorWorker => crate::matrix::DeploymentKind::CoordinatorWorker,
            },
            ..MatrixSpec::default()
        }
    }
}

/// One row in the shared correctness catalog.
#[derive(Debug, Clone, Copy)]
pub struct CatalogEntry {
    /// Stable id (matches test name prefix).
    pub id: &'static str,
    /// Happy or sad path.
    pub path: PathKind,
    /// Deployment matrix slice.
    pub deployment: CatalogDeployment,
    /// Scenario factory.
    pub spec: fn() -> ScenarioSpec,
    /// When true, the scenario runner must report a step error (timeout, etc.).
    pub expect_scenario_error: bool,
}

/// Storage adapters participating in the e2e matrix (mem active in PR CI).
#[must_use]
pub fn e2e_storage_backends() -> &'static [StorageAdapter] {
    &[StorageAdapter::Mem, StorageAdapter::Sqlite]
}

macro_rules! catalog_entries {
    ($deployment:expr) => {
        [
            entry(
                "scheduler_tick_smoke",
                PathKind::Happy,
                $deployment,
                ScenarioSpec::scheduler_tick_smoke,
            ),
            entry(
                "due_job_enqueue",
                PathKind::Happy,
                $deployment,
                ScenarioSpec::due_job_enqueue,
            ),
            entry(
                "script_run_success",
                PathKind::Happy,
                $deployment,
                ScenarioSpec::script_run_success,
            ),
            entry(
                "run_once_idempotent",
                PathKind::Happy,
                $deployment,
                ScenarioSpec::run_once_idempotent,
            ),
            entry(
                "telemetry_lifecycle",
                PathKind::Happy,
                $deployment,
                ScenarioSpec::telemetry_lifecycle,
            ),
            entry(
                "partition_due_filter",
                PathKind::Happy,
                $deployment,
                ScenarioSpec::partition_due_filter,
            ),
            entry(
                "not_due_no_enqueue",
                PathKind::Sad,
                $deployment,
                ScenarioSpec::not_due_no_enqueue,
            ),
            entry(
                "pause_resume_smoke",
                PathKind::Sad,
                $deployment,
                ScenarioSpec::pause_resume_smoke,
            ),
            entry(
                "script_run_failure",
                PathKind::Sad,
                $deployment,
                ScenarioSpec::script_run_failure,
            ),
            entry(
                "run_now_smoke",
                PathKind::Happy,
                $deployment,
                ScenarioSpec::run_now_smoke,
            ),
            entry(
                "job_revisions_smoke",
                PathKind::Happy,
                $deployment,
                ScenarioSpec::job_revisions_smoke,
            ),
            entry(
                "counting_exactly_once",
                PathKind::Happy,
                $deployment,
                ScenarioSpec::counting_exactly_once,
            ),
            entry_sad_error(
                "wait_run_timeout",
                $deployment,
                ScenarioSpec::wait_run_timeout,
            ),
        ]
    };
}

/// Full embedded deployment catalog (PR CI default).
static EMBEDDED_CATALOG: [CatalogEntry; 13] = catalog_entries!(CatalogDeployment::Embedded);

/// Embedded deployment catalog slice.
#[must_use]
pub fn embedded_catalog() -> &'static [CatalogEntry] {
    &EMBEDDED_CATALOG
}

/// Coordinator–worker split deployment catalog (full parity with embedded).
static COORDINATOR_CATALOG: [CatalogEntry; 13] =
    catalog_entries!(CatalogDeployment::CoordinatorWorker);

/// Coordinator–worker deployment catalog slice.
#[must_use]
pub fn coordinator_catalog() -> &'static [CatalogEntry] {
    &COORDINATOR_CATALOG
}

/// Back-compat alias for mem embedded catalog.
#[must_use]
pub fn mem_embedded_catalog() -> &'static [CatalogEntry] {
    embedded_catalog()
}

/// Back-compat alias for mem coordinator catalog.
#[must_use]
pub fn mem_coordinator_catalog() -> &'static [CatalogEntry] {
    coordinator_catalog()
}

const fn entry(
    id: &'static str,
    path: PathKind,
    deployment: CatalogDeployment,
    spec: fn() -> ScenarioSpec,
) -> CatalogEntry {
    CatalogEntry {
        id,
        path,
        deployment,
        spec,
        expect_scenario_error: false,
    }
}

const fn entry_sad_error(
    id: &'static str,
    deployment: CatalogDeployment,
    spec: fn() -> ScenarioSpec,
) -> CatalogEntry {
    CatalogEntry {
        id,
        path: PathKind::Sad,
        deployment,
        spec,
        expect_scenario_error: true,
    }
}

/// Run one catalog entry for the given storage adapter.
///
/// # Panics
///
/// Panics on bootstrap failure or scenario assertion mismatch.
pub async fn run_catalog_entry(entry: &CatalogEntry, storage: StorageAdapter) {
    let matrix = entry.deployment.matrix_spec(storage);
    if !extended_store_available(matrix.storage) {
        eprintln!(
            "catalog entry {}/{}: storage env not set — skipping",
            entry.id,
            matrix.storage.as_str()
        );
        return;
    }

    let mut session = BootstrapSession::new(matrix);
    session.install().await.expect("bootstrap install");
    let spec = (entry.spec)();
    let mut runner = ScenarioRunner::new(&mut session);
    let result = runner
        .run(&spec, RunMode::Correctness)
        .await
        .expect("scenario run");
    session.shutdown_embedded().await.expect("shutdown");

    if entry.expect_scenario_error {
        assert!(
            result.error.is_some(),
            "scenario {} expected error, got success",
            entry.id
        );
    } else {
        assert!(
            result.error.is_none(),
            "scenario {} failed: {:?}",
            entry.id,
            result.error
        );
    }
}

/// Look up a catalog row by id and deployment, then run it.
///
/// # Panics
///
/// Panics if `id` is not present in the deployment catalog.
pub async fn run_catalog_entry_by_id(
    id: &str,
    deployment: CatalogDeployment,
    storage: StorageAdapter,
) {
    let catalog = match deployment {
        CatalogDeployment::Embedded => embedded_catalog(),
        CatalogDeployment::CoordinatorWorker => coordinator_catalog(),
    };
    let entry = catalog
        .iter()
        .find(|e| e.id == id)
        .unwrap_or_else(|| panic!("unknown catalog entry id: {id}"));
    run_catalog_entry(entry, storage).await;
}
