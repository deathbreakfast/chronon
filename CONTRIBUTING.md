# Contributing to Chronon

## Development setup

1. Clone [unified-field-dev/chronon](https://github.com/unified-field-dev/chronon)
2. Rust **1.88+** (workspace `rust-version` / MSRV); CI installs `stable` (must be ≥ MSRV)
3. From repo root — **always use one cargo worker** (disk-friendly builds):

```bash
export CARGO_BUILD_JOBS=1
export CARGO_TARGET_DIR=target-chronon-extract

cargo test --workspace --exclude chronon-e2e --exclude chronon-bench
cargo test -p chronon-e2e -p chronon-axum -- --test-threads=1
cargo test -p chronon-backend-mem --tests
./scripts/coverage.sh --summary-only
cargo check -p uf-chronon --no-default-features
cargo check -p uf-chronon --features mem,telemetry-console,axum
cargo deny check
cargo clippy --workspace --all-targets -- -D warnings
cargo machete
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --exclude uf-chronon --no-deps
RUSTDOCFLAGS="-D warnings" cargo doc -p uf-chronon --all-features --no-deps
cargo test --doc -p chronon-core
cargo test --doc -p chronon-backend-mem
cargo test --doc -p chronon-backend-sql-common
cargo test --doc -p chronon-backend-postgres
cargo test --doc -p chronon-backend-sqlite
cargo test --doc -p chronon-backend-redis
cargo test --doc -p chronon-runtime
cargo test --doc -p chronon-scheduler
cargo test --doc -p chronon-executor
cargo test --doc -p chronon-axum
```

Run cargo commands **sequentially** — do not parallelize builds in multiple shells.

See [`docs/VERIFICATION.md`](docs/VERIFICATION.md) for the full pre-PR checklist and coverage baseline.

## Pull requests

- Keep changes scoped to one concern
- Add or update tests for behavior changes
- Run the verify commands above before opening a PR
- Document public API changes in rustdoc

## Code quality

### Sentrux (before finishing)

Record baseline `quality_signal` (currently **7424**) before changes; do not regress without justification.

1. `scan` with path set to the repository root
2. `check_rules` — zero violations (see `.sentrux/rules.toml`)
3. After module boundary changes: `dsm` with `format=stats`

### Module discipline

| Limit | Value |
|-------|-------|
| SLOC target | ≤ 400 code lines per file |
| SLOC hard stop | 450 code lines (rustdoc excluded) |
| Fan-out | ≤ 15 |
| Cyclomatic complexity | ≤ 25 per function |

- No god files or kitchen sinks — one responsibility per module
- No wildcard `pub use foo::*` in `lib.rs` or facade modules
- Keep `lib.rs` thin (mod declarations, named re-exports, crate docs)
- Exemplar: `chronon-axum/src/handlers.rs` — small per-route handlers

File SLOC is enforced via Sentrux + review judgment (no repo scripts). Use local `tokei` when sizing a split.

### Rustdoc

- `//!` landing pages on crate and submodule roots (getting started, guided modes, modules)
- `///` on every public item you add or change; add `# Examples` on major entry points
- No `#![allow(missing_docs)]` without explicit approval
- Verify: `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --exclude uf-chronon --no-deps`, then `cargo doc -p uf-chronon --all-features --no-deps`, and `cargo test --doc` on crates with examples
- Facade links feature-gated types; always document `uf-chronon` with `--all-features` (also set in `[package.metadata.docs.rs]`)

### Lint policy

- Clippy: pedantic + nursery, `-D warnings` in CI
- No new `#[allow(dead_code)]` or inline clippy allows — fix or use root `clippy.toml`
- `cargo deny check` — advisories, licenses, git sources (`deny.toml`)
- `cargo machete` — no unused dependencies

### Crate layering

```
chronon (facade) → chronon-runtime → chronon-{scheduler,executor} → chronon-core
chronon-backend-mem → chronon-core
chronon-backend-{postgres,sqlite} → chronon-backend-sql-common → chronon-core
chronon-backend-redis → chronon-backend-{postgres,sql-common} → chronon-core
chronon-axum → chronon-runtime
chronon-{testkit,e2e,bench} → runtime (internal only)
```

Lower layers must not import testkit, e2e, or bench crates.

## Definition of done

- [ ] Sentrux `scan` + `check_rules`: pass, 0 violations
- [ ] Sentrux `quality_signal`: ≥ baseline
- [ ] `cargo deny check`: pass
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`: clean
- [ ] `cargo test --workspace --exclude chronon-e2e --exclude chronon-bench`: all pass
- [ ] `cargo test -p chronon-e2e -p chronon-axum -- --test-threads=1`: all pass
- [ ] `./scripts/coverage.sh --summary-only`: no unexpected regression
- [ ] `cargo machete`: no unused deps
- [ ] `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --exclude uf-chronon --no-deps`: clean
- [ ] `RUSTDOCFLAGS="-D warnings" cargo doc -p uf-chronon --all-features --no-deps`: clean
- [ ] `cargo test --doc` on core/runtime/scheduler/executor/axum/backend-{mem,sql-common,postgres,sqlite,redis}: pass
- [ ] No file > 450 SLOC (code only) without splitting
- [ ] No new function with CC > 25
- [ ] Architectural boundaries in `.sentrux/rules.toml` preserved

## Adapter crates

Third-party persistence adapters belong in separate crates that implement `SchedulerStore` from `chronon-core`. Do not add host-specific persistence into upstream scheduler crates.

## Benchmarks

Pre-register experiments in [`chronon-bench/EXPERIMENTS.md`](chronon-bench/EXPERIMENTS.md) before running performance campaigns. Record results in the experiment log table.

```bash
cargo run -p chronon-bench -- run --experiment bm-ch0 --storage mem --deployment embedded
```
