//! In-memory [`ScriptRegistry`] with optional link-time inventory discovery.

use std::collections::HashMap;

use chronon_core::{ChrononError, Result};

use crate::descriptor::{InvokeFn, ScriptDescriptor};

struct StoredScript {
    name: String,
    invoke: InvokeFn,
    signature_json: String,
    signature_hash: u64,
}

/// In-memory script registry with optional link-time inventory discovery.
///
/// Populated at boot via [`Self::from_inventory`] / [`Self::register_from_inventory`] (from
/// `#[chronon::script]`) or manual [`Self::register`] calls. Read by the executor when a run is
/// claimed.
///
/// In Mode 2 (coordinator + worker), scripts must be registered on **worker** binaries —
/// that is where handlers execute. Prefer `ChrononBuilder::auto_registry()` so inventory is
/// collected automatically.
///
/// # Examples
///
/// Empty registry:
///
/// ```
/// use chronon_executor::ScriptRegistry;
///
/// let registry = ScriptRegistry::new();
/// assert!(registry.is_empty());
/// ```
///
/// Explicit register (daemon-style, without the macro):
///
/// ```
/// use chronon_core::{Result, ScriptContext};
/// use chronon_executor::{ScriptDescriptor, ScriptRegistry};
///
/// fn noop(
///     _ctx: Box<dyn ScriptContext>,
///     _params: serde_json::Value,
/// ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send>> {
///     Box::pin(async { Ok(()) })
/// }
///
/// let mut registry = ScriptRegistry::new();
/// registry.register(ScriptDescriptor::new("daemon-noop", noop));
/// assert!(registry.contains("daemon-noop"));
/// assert_eq!(registry.len(), 1);
/// ```
pub struct ScriptRegistry {
    scripts: HashMap<String, StoredScript>,
}

impl Default for ScriptRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ScriptRegistry {
    /// Creates an empty registry with no registered scripts.
    ///
    /// # Examples
    ///
    /// ```
    /// use chronon_executor::ScriptRegistry;
    ///
    /// let registry = ScriptRegistry::new();
    /// assert!(registry.is_empty());
    /// ```
    pub fn new() -> Self {
        Self {
            scripts: HashMap::new(),
        }
    }

    /// Populate from all `#[chronon::script]` descriptors linked into the binary.
    ///
    /// Equivalent to `ChrononBuilder::auto_registry()` wiring. Prefer this on Mode 2 workers.
    pub fn from_inventory() -> Self {
        let mut registry = Self::new();
        registry.register_from_inventory();
        registry
    }

    /// Register every script descriptor collected via link-time inventory.
    pub fn register_from_inventory(&mut self) {
        for desc in quark::inventory::iter::<ScriptDescriptor> {
            self.register(ScriptDescriptor::with_signature(
                desc.name,
                desc.invoke,
                desc.signature_json,
                desc.signature_hash,
            ));
        }
    }

    /// Inserts or replaces a script descriptor keyed by [`ScriptDescriptor::name`].
    ///
    /// Duplicate names overwrite the previous handler without error.
    pub fn register(&mut self, desc: ScriptDescriptor) {
        self.scripts.insert(
            desc.name.to_string(),
            StoredScript {
                name: desc.name.to_string(),
                invoke: desc.invoke,
                signature_json: desc.signature_json.to_string(),
                signature_hash: desc.signature_hash,
            },
        );
    }

    /// Returns a borrowed view of the script named `name`, if registered.
    pub fn get(&self, name: &str) -> Option<ScriptDescriptorRef<'_>> {
        self.scripts.get(name).map(|s| ScriptDescriptorRef {
            name: s.name.as_str(),
            invoke: s.invoke,
            signature_json: s.signature_json.as_str(),
            signature_hash: s.signature_hash,
        })
    }

    /// Like [`Self::get`], but returns [`ChrononError::ScriptNotFound`] when absent.
    pub fn get_or_err(&self, name: &str) -> Result<ScriptDescriptorRef<'_>> {
        self.get(name)
            .ok_or_else(|| ChrononError::ScriptNotFound(name.to_string()))
    }

    /// Returns the number of registered scripts.
    pub fn len(&self) -> usize {
        self.scripts.len()
    }

    /// Returns `true` when no scripts are registered.
    pub fn is_empty(&self) -> bool {
        self.scripts.is_empty()
    }

    /// Returns `true` when a script with the given name is registered.
    pub fn contains(&self, name: &str) -> bool {
        self.scripts.contains_key(name)
    }

    /// Returns all registered scripts sorted by name.
    pub fn list(&self) -> Vec<ScriptDescriptorRef<'_>> {
        let mut v: Vec<_> = self.scripts.values().map(stored_to_ref).collect();
        v.sort_by_key(|d| d.name);
        v
    }
}

/// Borrowed view of a registered script suitable for invocation.
#[derive(Clone, Copy)]
pub struct ScriptDescriptorRef<'a> {
    /// Registered script name (matches job `script_name`).
    pub name: &'a str,
    /// Handler function pointer collected from inventory or manual registration.
    pub invoke: InvokeFn,
    /// JSON schema string for handler parameters (from `chronon-macros` when available).
    pub signature_json: &'a str,
    /// Compile-time hash of the signature for drift detection.
    pub signature_hash: u64,
}

fn stored_to_ref(s: &StoredScript) -> ScriptDescriptorRef<'_> {
    ScriptDescriptorRef {
        name: s.name.as_str(),
        invoke: s.invoke,
        signature_json: s.signature_json.as_str(),
        signature_hash: s.signature_hash,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptor::ScriptDescriptor;
    use chronon_core::{Result, ScriptContext};
    use serde_json::Value;
    use std::future::Future;
    use std::pin::Pin;

    fn stub_invoke(
        _ctx: Box<dyn ScriptContext>,
        _params: Value,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send>> {
        Box::pin(async { Ok(()) })
    }

    #[test]
    fn register_and_lookup() {
        let mut reg = ScriptRegistry::new();
        reg.register(ScriptDescriptor::new("hello", stub_invoke));
        assert!(reg.contains("hello"));
        assert_eq!(reg.len(), 1);
    }
}
