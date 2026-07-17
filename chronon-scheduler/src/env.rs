//! Environment flag helpers for scheduler loops.

/// Returns `true` when `name` is set to `1` or `true` (case-insensitive).
///
/// Used by the tick loop to honor dev kill-switches such as `CHRONON_DISABLE_COORDINATOR`.
/// Unset or unrecognized values return `false`.
pub fn env_flag(name: &str) -> bool {
    std::env::var(name).is_ok_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
}
