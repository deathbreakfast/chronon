//! Sanitize operator-visible error strings (runs, HTTP envelopes).

/// Maximum length of persisted / logged handler error messages.
pub const MAX_ERROR_MESSAGE_CHARS: usize = 512;

/// Truncate and strip obvious secret-looking substrings from an error message.
///
/// Does not attempt full redaction; callers must still avoid embedding params/actor JSON
/// in errors. Used before persisting run `error_message` / stderr and returning HTTP errors.
#[must_use]
pub fn sanitize_error_message(raw: &str) -> String {
    let mut out = raw.replace('\0', "");
    for needle in [
        "password=",
        "Password=",
        "secret=",
        "Secret=",
        "token=",
        "Token=",
        "Bearer ",
        "authorization:",
    ] {
        if let Some(idx) = out.find(needle) {
            let end = (idx + needle.len() + 8).min(out.len());
            out.replace_range(idx..end, &format!("{needle}[redacted]"));
        }
    }
    if out.chars().count() > MAX_ERROR_MESSAGE_CHARS {
        out = out.chars().take(MAX_ERROR_MESSAGE_CHARS).collect();
        out.push('…');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncates_long_messages() {
        let long = "x".repeat(800);
        let s = sanitize_error_message(&long);
        assert!(s.chars().count() <= MAX_ERROR_MESSAGE_CHARS + 1);
        assert!(s.ends_with('…'));
    }

    #[test]
    fn redacts_password_prefix() {
        let s = sanitize_error_message("db failed password=hunter2 more");
        assert!(s.contains("[redacted]"));
        assert!(!s.contains("hunter2"));
    }
}
