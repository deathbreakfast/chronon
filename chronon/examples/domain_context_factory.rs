//! Custom [`ContextFactory`] that injects domain provenance (tenant/user/region) and fails
//! closed on incomplete actor JSON — **Integrating the host** (identity).
//!
//! ```bash
//! cargo run -p uf-chronon --example domain_context_factory --features mem
//! ```
//!
//! Mirrors the custom-factory sketch in `chronon-core` [`ContextFactory`] rustdoc
//! (`chronon-core/src/context.rs`): production hosts map actor JSON to sessions, permissions,
//! or — as here — multi-tenant provenance, and reject payloads missing required fields rather
//! than falling back to an ambiguous default.

use std::sync::Arc;

use chrono::{Duration, Utc};
use chronon::prelude::*;
use chronon_backend_mem::InMemorySchedulerStore;
use chronon_core::IdentityError;
use chronon_executor::{execute_script, ExecuteScriptRequest};
use serde_json::{json, Value};

/// Execution context whose label carries tenant/user/region provenance for logs.
struct DomainContext {
    label: String,
    actor_json: Value,
}

impl ScriptContext for DomainContext {
    fn label(&self) -> &str {
        &self.label
    }

    fn actor_json(&self) -> &Value {
        &self.actor_json
    }
}

/// Rebuilds domain identity from actor JSON captured at schedule time.
///
/// Fails closed ([`ChrononError::Identity`]) when `tenant` or `user` is missing — untrusted or
/// incomplete payloads must not silently execute with partial identity.
#[derive(Debug, Default, Clone, Copy)]
struct DomainContextFactory;

impl ContextFactory for DomainContextFactory {
    fn build(&self, actor_json: &Value) -> chronon::Result<Box<dyn ScriptContext>> {
        let tenant = actor_json
            .get("tenant")
            .and_then(Value::as_str)
            .ok_or_else(|| IdentityError("actor_json missing \"tenant\"".into()))?;
        let user = actor_json
            .get("user")
            .and_then(Value::as_str)
            .ok_or_else(|| IdentityError("actor_json missing \"user\"".into()))?;
        let region = actor_json
            .get("region")
            .and_then(Value::as_str)
            .unwrap_or("unspecified");

        Ok(Box::new(DomainContext {
            label: format!("tenant={tenant} user={user} region={region}"),
            actor_json: actor_json.clone(),
        }))
    }
}

#[chronon::script(name = "domain_report")]
#[allow(clippy::unused_async)] // script handlers are always async
async fn domain_report(ctx: Box<dyn ScriptContext>) -> chronon::Result<()> {
    eprintln!("domain_report running as {}", ctx.label());
    Ok(())
}

#[tokio::main]
async fn main() -> chronon::Result<()> {
    let store = Arc::new(InMemorySchedulerStore::new());
    let chronon = ChrononBuilder::new()
        .scheduler_store(store)
        .context_factory(Arc::new(DomainContextFactory))
        .embedded()
        .auto_registry()
        .build()?;

    let job = JobBuilder::new(&domain_report())
        .name("domain-report-job")
        .run_once_at(Utc::now() - Duration::seconds(60))
        .with_actor_json(json!({ "tenant": "acme", "user": "alice", "region": "us-east" }))
        .build()?;
    chronon.coordinator_service().upsert_job(job).await?;

    chronon.scheduler.init_partitions().await;
    let tick = chronon.tick_once().await?;
    assert!(tick.enqueued >= 1, "expected at least one enqueued run");

    // Drive the same invoke path a worker uses to prove allow / fail-closed deny in-process.
    let executor = chronon.executor();
    let allowed = execute_script(ExecuteScriptRequest {
        registry: &executor.registry,
        context_factory: &executor.context_factory,
        telemetry: &executor.telemetry,
        script_name: "domain_report",
        actor_json: &json!({ "tenant": "acme", "user": "alice", "region": "us-east" }),
        params_json: json!({}),
        job_name: "domain-report-job",
        run_id: "demo-allowed",
    })
    .await;
    assert!(allowed.result.is_ok(), "expected domain context to build");

    let denied = execute_script(ExecuteScriptRequest {
        registry: &executor.registry,
        context_factory: &executor.context_factory,
        telemetry: &executor.telemetry,
        script_name: "domain_report",
        actor_json: &json!({ "user": "alice" }),
        params_json: json!({}),
        job_name: "domain-report-job",
        run_id: "demo-denied",
    })
    .await;
    assert!(
        matches!(denied.result, Err(ChrononError::Identity(_))),
        "expected fail-closed identity error, got {:?}",
        denied.result
    );

    eprintln!(
        "domain_context_factory: tick enqueued {} run(s); allow ok, missing-tenant denied (fail closed)",
        tick.enqueued
    );
    Ok(())
}
