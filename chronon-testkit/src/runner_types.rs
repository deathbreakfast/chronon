//! Shared types for scenario runner and step dispatch (keeps modules acyclic).

/// Driver mode: assert on outcomes vs collect timings only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunMode {
    /// Run assertion steps and fail on mismatches.
    Correctness,
    /// Skip assertions; record per-step timings for bench reports.
    Benchmark,
}

/// Per-step timing samples (milliseconds).
#[derive(Debug, Clone)]
pub struct StepTiming {
    /// Index of the step within the scenario.
    pub step_index: usize,
    /// Short operation label (for example `"tick"`).
    pub op: String,
    /// Elapsed samples in milliseconds for this step.
    pub samples_ms: Vec<f64>,
}
