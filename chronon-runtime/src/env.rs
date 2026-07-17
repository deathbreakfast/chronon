//! Environment flag helpers.

/// True when `name` is set to `1` or `true` (case-insensitive); false if unset or other values.
///
/// Used by the worker loop for `CHRONON_DISABLE_WORKER` and dev interactive mode.
pub fn env_flag(name: &str) -> bool {
    std::env::var(name).is_ok_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
}
