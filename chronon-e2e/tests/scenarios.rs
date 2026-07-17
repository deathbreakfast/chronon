//! Scenario correctness tests over the CI matrix slice.
//!
//! Distributed multi-worker smokes live in `distributed_smoke.rs` / the
//! `matrix_distributed_scenario_suite!` helper — keep them out of this binary so
//! `cargo test --test scenarios -- --ignored` stays bounded for PR durable CI.

chronon_testkit::matrix_scenario_suite!();

chronon_testkit::matrix_coordinator_scenario_suite!();

chronon_testkit::matrix_sqlite_scenario_suite!();

chronon_testkit::matrix_sqlite_coordinator_suite!();

chronon_testkit::matrix_postgres_scenario_suite!();

chronon_testkit::matrix_postgres_coordinator_suite!();

chronon_testkit::matrix_postgres_redis_scenario_suite!();

chronon_testkit::matrix_postgres_redis_coordinator_suite!();
