//! Script descriptor for auto-registration via inventory.

use std::future::Future;
use std::pin::Pin;

use chronon_core::{Result, ScriptContext};
use serde_json::Value;

/// Type alias for the script invocation function.
///
/// Registered via [`ScriptDescriptor`] and called by [`crate::execute_script`] after
/// context build; must be `Send` because runs execute on the tokio runtime.
pub type InvokeFn =
    fn(Box<dyn ScriptContext>, Value) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>>;

/// Descriptor for a registered script.
///
/// Collected at link time by `quark::inventory` for `#[chronon::script]` handlers or
/// built manually in tests via [`Self::new`].
pub struct ScriptDescriptor {
    /// Unique script name.
    pub name: &'static str,
    /// Function to invoke the script with deserialized parameters.
    pub invoke: InvokeFn,
    /// JSON schema for parameters (computed at compile time by `chronon-macros`).
    pub signature_json: &'static str,
    /// Hash of the signature for version checking.
    pub signature_hash: u64,
}

impl ScriptDescriptor {
    /// Create a descriptor with placeholder signature metadata.
    pub const fn new(name: &'static str, invoke: InvokeFn) -> Self {
        Self {
            name,
            invoke,
            signature_json: "{}",
            signature_hash: 0,
        }
    }

    /// Create a descriptor with full signature information.
    pub const fn with_signature(
        name: &'static str,
        invoke: InvokeFn,
        signature_json: &'static str,
        signature_hash: u64,
    ) -> Self {
        Self {
            name,
            invoke,
            signature_json,
            signature_hash,
        }
    }
}

impl std::fmt::Debug for ScriptDescriptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScriptDescriptor")
            .field("name", &self.name)
            .field("signature_json", &self.signature_json)
            .field("signature_hash", &self.signature_hash)
            .field("invoke", &"<fn>")
            .finish()
    }
}

quark::inventory::collect!(ScriptDescriptor);

impl quark::Registrable for ScriptDescriptor {
    fn registry_key(&self) -> &str {
        self.name
    }
}
