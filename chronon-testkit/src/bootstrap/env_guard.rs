//! Scoped process environment overrides for matrix bootstrap.

/// Restores a prior env value (or removes the key) on drop.
pub(crate) struct EnvGuard {
    key: &'static str,
    previous: Option<String>,
}

impl EnvGuard {
    /// Set `key` to `value`, remembering the prior value for restore on drop.
    pub fn set(key: &'static str, value: &str) -> Self {
        let previous = std::env::var(key).ok();
        std::env::set_var(key, value);
        Self { key, previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        if let Some(ref value) = self.previous {
            std::env::set_var(self.key, value);
        } else {
            std::env::remove_var(self.key);
        }
    }
}
