//! Per-run tracing capture for `stdout_text` / `stderr_text` on Chronon runs.
//!
//! Install [`ChrononLogCapture`] as a `tracing` layer (hosts typically compose it into
//! their `tracing_subscriber` registry). During script execution, enter a
//! [`CaptureScope`] so events are buffered and drained into [`CapturedLogs`] on
//! every terminal outcome — including failures.

use std::sync::Arc;

use parking_lot::Mutex;
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::registry::LookupSpan;

/// Default capture budget (1 MiB) matching Chronon DESIGN §12.
pub const DEFAULT_MAX_CAPTURE_BYTES: usize = 1_000_000;

/// Captured stdout/stderr text ready to persist on a Chronon run row.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CapturedLogs {
    /// Info/debug-level lines (and unclassified events).
    pub stdout_text: Option<String>,
    /// Warn/error-level lines (and failure messages).
    pub stderr_text: Option<String>,
}

impl CapturedLogs {
    /// True when either stream has non-empty text.
    #[must_use]
    pub fn has_text(&self) -> bool {
        self.stdout_text.as_ref().is_some_and(|s| !s.is_empty())
            || self.stderr_text.as_ref().is_some_and(|s| !s.is_empty())
    }

    /// Ensure `stderr_text` includes `message` when missing or empty.
    pub fn ensure_stderr_message(&mut self, message: &str) {
        if message.is_empty() {
            return;
        }
        match &mut self.stderr_text {
            Some(existing) if !existing.is_empty() => {
                if !existing.contains(message) {
                    existing.push('\n');
                    existing.push_str(message);
                }
            }
            _ => self.stderr_text = Some(message.to_string()),
        }
    }
}

#[derive(Debug)]
struct CaptureInner {
    max_bytes: usize,
    used: usize,
    stdout: String,
    stderr: String,
    /// When false, the layer ignores events (no active [`CaptureScope`]).
    active: bool,
}

impl CaptureInner {
    fn append(&mut self, to_stderr: bool, line: &str) {
        if line.is_empty() || self.used >= self.max_bytes {
            return;
        }
        let budget = self.max_bytes.saturating_sub(self.used);
        let chunk = if line.len() > budget {
            &line[..budget]
        } else {
            line
        };
        let dest = if to_stderr {
            &mut self.stderr
        } else {
            &mut self.stdout
        };
        if !dest.is_empty() {
            if self.used + 1 > self.max_bytes {
                return;
            }
            dest.push('\n');
            self.used += 1;
        }
        let remain = self.max_bytes.saturating_sub(self.used);
        let take = chunk.len().min(remain);
        dest.push_str(&chunk[..take]);
        self.used += take;
    }

    fn drain(&mut self) -> CapturedLogs {
        let stdout = std::mem::take(&mut self.stdout);
        let stderr = std::mem::take(&mut self.stderr);
        self.used = 0;
        CapturedLogs {
            stdout_text: if stdout.is_empty() {
                None
            } else {
                Some(stdout)
            },
            stderr_text: if stderr.is_empty() {
                None
            } else {
                Some(stderr)
            },
        }
    }
}

/// Shared capture state for a [`ChrononLogCapture`] layer and [`CaptureScope`]s.
#[derive(Clone, Debug)]
pub struct ChrononLogCapture {
    inner: Arc<Mutex<CaptureInner>>,
}

impl ChrononLogCapture {
    /// Create a capture layer with a byte budget for buffered text.
    #[must_use]
    pub fn new(max_bytes: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(CaptureInner {
                max_bytes: max_bytes.max(1),
                used: 0,
                stdout: String::new(),
                stderr: String::new(),
                active: false,
            })),
        }
    }

    /// Begin buffering events until [`CaptureScope`] is dropped / [`CaptureScope::finish`].
    #[must_use]
    pub fn enter(&self) -> CaptureScope {
        {
            let mut g = self.inner.lock();
            g.active = true;
            g.stdout.clear();
            g.stderr.clear();
            g.used = 0;
        }
        CaptureScope {
            inner: Arc::clone(&self.inner),
            finished: false,
        }
    }
}

/// RAII scope that enables capture for the current run and drains on finish.
#[derive(Debug)]
pub struct CaptureScope {
    inner: Arc<Mutex<CaptureInner>>,
    finished: bool,
}

impl CaptureScope {
    /// Disable capture and return buffered logs (safe to call once; further calls are empty).
    pub fn finish(mut self) -> CapturedLogs {
        self.finished = true;
        let mut g = self.inner.lock();
        g.active = false;
        g.drain()
    }
}

impl Drop for CaptureScope {
    fn drop(&mut self) {
        if !self.finished {
            let mut g = self.inner.lock();
            g.active = false;
            let _ = g.drain();
        }
    }
}

impl<S> Layer<S> for ChrononLogCapture
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let mut g = self.inner.lock();
        if !g.active {
            return;
        }
        let mut visitor = MessageVisitor::default();
        event.record(&mut visitor);
        let Some(msg) = visitor.message else {
            return;
        };
        let to_stderr = matches!(*event.metadata().level(), Level::ERROR | Level::WARN);
        g.append(to_stderr, &msg);
    }
}

#[derive(Default)]
struct MessageVisitor {
    message: Option<String>,
}

impl tracing::field::Visit for MessageVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = Some(format!("{value:?}").trim_matches('"').to_string());
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.message = Some(value.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::Registry;

    #[test]
    fn drain_empty_when_no_events() {
        let capture = ChrononLogCapture::new(1024);
        let scope = capture.enter();
        let logs = scope.finish();
        assert!(!logs.has_text());
    }

    #[test]
    fn captures_info_to_stdout_and_warn_to_stderr() {
        let capture = ChrononLogCapture::new(1024);
        let subscriber = Registry::default().with(capture.clone());
        let dispatch = tracing::dispatcher::Dispatch::new(subscriber);
        let logs = tracing::dispatcher::with_default(&dispatch, || {
            let scope = capture.enter();
            tracing::info!("hello stdout");
            tracing::warn!("hello stderr");
            scope.finish()
        });
        assert_eq!(logs.stdout_text.as_deref(), Some("hello stdout"));
        assert_eq!(logs.stderr_text.as_deref(), Some("hello stderr"));
    }

    #[test]
    fn respects_max_bytes() {
        let capture = ChrononLogCapture::new(8);
        let subscriber = Registry::default().with(capture.clone());
        let dispatch = tracing::dispatcher::Dispatch::new(subscriber);
        let logs = tracing::dispatcher::with_default(&dispatch, || {
            let scope = capture.enter();
            tracing::info!("0123456789abcdef");
            scope.finish()
        });
        let stdout = logs.stdout_text.expect("stdout");
        assert!(stdout.len() <= 8);
    }

    #[test]
    fn ensure_stderr_message_fills_empty() {
        let mut logs = CapturedLogs::default();
        logs.ensure_stderr_message("boom");
        assert_eq!(logs.stderr_text.as_deref(), Some("boom"));
    }
}
