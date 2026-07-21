//! Shared scenario executor for e2e (correctness) and bench (timings).

use anyhow::{bail, Result};
use chronon_scheduler::TickResult;

use crate::bootstrap::BootstrapSession;
use crate::runner_steps;
use crate::runner_types::{RunMode, StepTiming};
use crate::scenario::{ScenarioSpec, ScenarioStep};

/// Outcome of running one [`ScenarioSpec`].
#[derive(Debug, Clone)]
pub struct ScenarioResult {
    /// Scenario identifier from the spec.
    pub scenario_id: String,
    /// Matrix slug from the bootstrapped session.
    pub matrix_slug: String,
    /// Benchmark-mode timing samples per step.
    pub step_timings: Vec<StepTiming>,
    /// Last tick result when a tick step ran.
    pub last_tick: Option<TickResult>,
    /// First step error message, if any.
    pub error: Option<String>,
}

/// Executes declarative scenarios against a bootstrapped session.
pub struct ScenarioRunner<'a> {
    session: &'a mut BootstrapSession,
}

impl<'a> ScenarioRunner<'a> {
    /// Bind a runner to an installed [`BootstrapSession`].
    pub fn new(session: &'a mut BootstrapSession) -> Self {
        Self { session }
    }

    /// Run all steps in `spec`, honoring `mode` for assertions vs timings.
    pub async fn run(&mut self, spec: &ScenarioSpec, mode: RunMode) -> Result<ScenarioResult> {
        if !self.session.is_ready() {
            bail!("BootstrapSession::install must succeed before running scenarios");
        }

        let matrix_slug = self.session.matrix().report_slug();
        let mut step_timings = Vec::new();
        let mut last_tick = None;
        let mut result = ScenarioResult {
            scenario_id: spec.id.clone(),
            matrix_slug,
            step_timings: Vec::new(),
            last_tick: None,
            error: None,
        };

        for (step_index, step) in spec.steps.iter().enumerate() {
            let step_result = self
                .run_step(step_index, step, mode, &mut step_timings, &mut last_tick)
                .await;
            if let Err(e) = step_result {
                result.error = Some(e.to_string());
                result.step_timings = step_timings;
                result.last_tick = last_tick;
                return Ok(result);
            }
        }

        result.step_timings = step_timings;
        result.last_tick = last_tick;
        Ok(result)
    }

    async fn run_step(
        &mut self,
        step_index: usize,
        step: &ScenarioStep,
        mode: RunMode,
        step_timings: &mut Vec<StepTiming>,
        last_tick: &mut Option<TickResult>,
    ) -> Result<()> {
        if runner_steps::is_assertion_step(step) {
            return runner_steps::run_assertion_step(self.session, step, mode, last_tick.as_ref())
                .await;
        }

        runner_steps::run_mutation_step(
            self.session,
            step_index,
            step,
            mode,
            step_timings,
            last_tick,
        )
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matrix::MatrixSpec;

    #[tokio::test]
    async fn scheduler_tick_smoke_ci_slice() {
        let mut session = BootstrapSession::new(MatrixSpec::ci_mem_embedded());
        session.install().await.expect("install");
        let mut runner = ScenarioRunner::new(&mut session);
        let result = runner
            .run(&ScenarioSpec::scheduler_tick_smoke(), RunMode::Correctness)
            .await
            .expect("run");
        assert!(result.error.is_none(), "{:?}", result.error);
    }
}
