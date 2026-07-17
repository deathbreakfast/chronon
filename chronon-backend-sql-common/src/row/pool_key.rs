//! Run pool key normalization for queue routing.

/// Resolve the Redis / queue pool key for a run (`"general"` when unset).
pub fn run_pool_key(pool_id: Option<&str>) -> &str {
    match pool_id {
        None | Some("general") => "general",
        Some(other) => other,
    }
}

#[cfg(test)]
mod tests {
    use super::run_pool_key;

    #[test]
    fn run_pool_key_defaults_to_general() {
        assert_eq!(run_pool_key(None), "general");
        assert_eq!(run_pool_key(Some("general")), "general");
    }

    #[test]
    fn run_pool_key_preserves_custom_pool() {
        assert_eq!(run_pool_key(Some("batch")), "batch");
    }
}
