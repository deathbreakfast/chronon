//! Abort Chronon run-lease renew tasks on drop.

use tokio::task::JoinHandle;

/// Aborts the renew task when dropped so a panicked worker slot cannot keep a lease alive.
pub(crate) struct AbortOnDrop(Option<JoinHandle<()>>);

impl AbortOnDrop {
    pub(crate) fn new(handle: JoinHandle<()>) -> Self {
        Self(Some(handle))
    }
}

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        if let Some(handle) = self.0.take() {
            handle.abort();
        }
    }
}
