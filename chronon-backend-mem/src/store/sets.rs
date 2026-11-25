//! Child sets, shard sets, run outputs and active-run counts.

use chrono::{DateTime, Utc};
use serde_json::Value;

use chronon_core::error::ChrononError;
use chronon_core::models::{ChildOutcome, ChildSet, Run, RunStatus, ShardSet};
use chronon_core::Result;

use super::InMemorySchedulerStore;

pub(super) fn set_run_outputs(
    store: &InMemorySchedulerStore,
    run_id: &str,
    outputs: &Value,
) -> Result<()> {
    let mut runs = store.runs.write();
    let run = runs
        .get_mut(run_id)
        .ok_or_else(|| ChrononError::RunNotFound(run_id.to_string()))?;
    run.outputs_json = outputs.clone();
    Ok(())
}

pub(super) fn count_active_runs_for_job(
    store: &InMemorySchedulerStore,
    job_id: &str,
) -> Result<i64> {
    let count = store
        .runs
        .read()
        .values()
        .filter(|r| r.job_id.as_deref() == Some(job_id))
        .filter(|r| matches!(r.status, RunStatus::Claimed | RunStatus::Running))
        .count();
    i64::try_from(count).map_err(|e| ChrononError::Internal(e.to_string()))
}

/// Insert the set and every run under both write locks, after checking that no id is taken,
/// so a failure leaves nothing behind.
pub(super) fn create_child_set_with_runs(
    store: &InMemorySchedulerStore,
    set: &ChildSet,
    new_runs: &[Run],
) -> Result<()> {
    let mut sets = store.child_sets.write();
    let mut runs = store.runs.write();
    if sets.contains_key(&set.set_id) {
        return Err(ChrononError::storage(format!(
            "duplicate set_id: {}",
            set.set_id
        )));
    }
    let mut seen = std::collections::HashSet::with_capacity(new_runs.len());
    for run in new_runs {
        if runs.contains_key(&run.run_id) || !seen.insert(run.run_id.as_str()) {
            return Err(ChrononError::storage(format!(
                "duplicate run_id: {}",
                run.run_id
            )));
        }
    }
    sets.insert(set.set_id.clone(), set.clone());
    for run in new_runs {
        runs.insert(run.run_id.clone(), run.clone());
    }
    Ok(())
}

pub(super) fn get_child_set(
    store: &InMemorySchedulerStore,
    set_id: &str,
) -> Result<Option<ChildSet>> {
    Ok(store.child_sets.read().get(set_id).cloned())
}

pub(super) fn list_child_sets_for_run(
    store: &InMemorySchedulerStore,
    parent_run_id: &str,
) -> Result<Vec<ChildSet>> {
    let mut sets: Vec<_> = store
        .child_sets
        .read()
        .values()
        .filter(|s| s.parent_run_id == parent_run_id)
        .cloned()
        .collect();
    sets.sort_by(|a, b| {
        a.created_at
            .cmp(&b.created_at)
            .then_with(|| a.set_id.cmp(&b.set_id))
    });
    Ok(sets)
}

pub(super) fn record_child_result(
    store: &InMemorySchedulerStore,
    set_id: &str,
    outcome: ChildOutcome,
) -> Result<ChildSet> {
    let mut sets = store.child_sets.write();
    let set = sets
        .get_mut(set_id)
        .ok_or_else(|| ChrononError::storage(format!("child set not found: {set_id}")))?;
    let counter = match outcome {
        ChildOutcome::Succeeded => &mut set.succeeded,
        ChildOutcome::Failed => &mut set.failed,
        ChildOutcome::Skipped => &mut set.skipped,
    };
    *counter = counter.saturating_add(1);
    Ok(set.clone())
}

pub(super) fn settle_child_set(
    store: &InMemorySchedulerStore,
    set_id: &str,
    at: DateTime<Utc>,
) -> Result<()> {
    if let Some(set) = store.child_sets.write().get_mut(set_id) {
        set.settled_at.get_or_insert(at);
    }
    Ok(())
}

pub(super) fn create_shard_set(store: &InMemorySchedulerStore, set: &ShardSet) -> Result<()> {
    let mut sets = store.shard_sets.write();
    if sets.contains_key(&set.set_id) {
        return Err(ChrononError::storage(format!(
            "duplicate set_id: {}",
            set.set_id
        )));
    }
    sets.insert(set.set_id.clone(), set.clone());
    Ok(())
}

pub(super) fn get_shard_set(
    store: &InMemorySchedulerStore,
    set_id: &str,
) -> Result<Option<ShardSet>> {
    Ok(store.shard_sets.read().get(set_id).cloned())
}

pub(super) fn update_shard_set(
    store: &InMemorySchedulerStore,
    set: &ShardSet,
    expected_next_start: &Value,
) -> Result<bool> {
    let mut sets = store.shard_sets.write();
    let Some(stored) = sets.get_mut(&set.set_id) else {
        return Ok(false);
    };
    if stored.next_start != *expected_next_start {
        return Ok(false);
    }
    stored.next_start.clone_from(&set.next_start);
    stored.end.clone_from(&set.end);
    stored.rows_per_shard = set.rows_per_shard;
    stored.paused = set.paused;
    Ok(true)
}

pub(super) fn set_shard_checkpoint(
    store: &InMemorySchedulerStore,
    set_id: &str,
    shard_index: u32,
    value: &Value,
) -> Result<()> {
    let mut sets = store.shard_sets.write();
    let set = sets
        .get_mut(set_id)
        .ok_or_else(|| ChrononError::storage(format!("shard set not found: {set_id}")))?;
    if !set.checkpoints_json.is_object() {
        set.checkpoints_json = Value::Object(serde_json::Map::default());
    }
    if let Value::Object(map) = &mut set.checkpoints_json {
        map.insert(shard_index.to_string(), value.clone());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use chronon_core::models::{ChildOutcome, ChildSet};
    use chronon_core::store::SchedulerStore;

    use crate::InMemorySchedulerStore;

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn record_child_result_never_loses_a_count() {
        let store = Arc::new(InMemorySchedulerStore::new());
        let set = ChildSet::new("parent", "child-job", 400);
        store.create_child_set_with_runs(&set, &[]).await.unwrap();
        let mut joins = Vec::new();
        for task in 0..8 {
            let store = Arc::clone(&store);
            let set_id = set.set_id.clone();
            joins.push(tokio::spawn(async move {
                for i in 0..25 {
                    let outcome = if (task + i) % 2 == 0 {
                        ChildOutcome::Succeeded
                    } else {
                        ChildOutcome::Failed
                    };
                    store.record_child_result(&set_id, outcome).await.unwrap();
                }
            }));
        }
        for join in joins {
            join.await.unwrap();
        }
        let stored = store.get_child_set(&set.set_id).await.unwrap().unwrap();
        assert_eq!(stored.succeeded + stored.failed, 200);
        assert_eq!(stored.succeeded, 100);
    }
}
