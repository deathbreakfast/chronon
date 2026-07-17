# chronon-testkit

Matrix bootstrap and scenario helpers for e2e/bench.

## API

| Type | Role |
|------|------|
| `MatrixSpec` | Storage × deployment × topology × telemetry dimensions |
| `BootstrapSession` | Installs mem store + builds `Chronon` for embedded or coordinator-only tick paths |
| `SplitHandle` | In-process coordinator + worker pair on shared mem store |
| `ScenarioSpec` / `ScenarioStep` | Declarative scenario catalog (JSON-serializable) |
| `ScenarioRunner` | Executes scenarios in `RunMode::Correctness` or `Benchmark` |
| `RecordingSink` (via `chronon-telemetry`) | In-memory telemetry assertions |

### CI scenario catalog

| ID | Covers |
|----|--------|
| `scheduler-tick-smoke` | Empty store tick |
| `due-job-enqueue` | Due cron → enqueued run |
| `script-run-success` | Embedded worker executes noop script |
| `run-once-idempotent` | RunOnce fires once |
| `telemetry-lifecycle` | Scheduler tick + run completion counters |
| `partition-due-filter` | Partition-scoped due query |
| `not-due-no-enqueue` | Future cron skipped on tick (sad path) |
| `pause-resume-smoke` | Paused job skipped; resume re-enables (sad + happy) |
| `script-run-failure` | Failing probe → `RunStatus::Failed` (sad path) |
| `run-now-smoke` | Manual job triggered via coordinator |
| `job-revisions-smoke` | Revision row appended on coordinator upsert |
| `counting-exactly-once` | Counting probe invoked once per run |
| `wait-run-timeout` | Wrong terminal wait times out **(sad)** |

Leader election is covered by [`run_store_contract`](src/store_contract.rs) and `chronon-scheduler/tests/leader_integration.rs`, not E2E scenarios.

Catalog source: [`catalog.rs`](src/catalog.rs) ([`invoke_catalog_scenario_ids!`](src/catalog.rs)). Matrix expansion: [`macros.rs`](src/macros.rs).

### Store contract

[`run_store_contract`](src/store_contract.rs) implements the shared [`SchedulerStore`](https://docs.rs/chronon-core/latest/chronon_core/store/trait.SchedulerStore.html) port checks every backend adapter must pass before matrix E2E expansion. Mem backend runs it in `chronon-backend-mem/tests/store_contract.rs`.

## Verify

```bash
export CARGO_BUILD_JOBS=1 CARGO_TARGET_DIR=../../target-chronon-extract
cargo test -p chronon-testkit
```
