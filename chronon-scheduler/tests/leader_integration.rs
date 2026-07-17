//! Scheduler leader election integration (store + leader module).

use std::sync::Arc;

use chronon_backend_mem::InMemorySchedulerStore;
use chronon_core::store::SchedulerStore;
use chronon_scheduler::{am_i_leader, try_acquire_leader};

#[tokio::test]
async fn leader_module_blocks_second_instance() {
    let store: Arc<dyn SchedulerStore> = Arc::new(InMemorySchedulerStore::new());

    assert!(try_acquire_leader(&store, "coord-a").await.unwrap());
    assert!(!try_acquire_leader(&store, "coord-b").await.unwrap());
    assert!(am_i_leader(&store, "coord-a").await.unwrap());
    assert!(!am_i_leader(&store, "coord-b").await.unwrap());
}
