//! Test utility functions for pi-pipeline

use pipeline::core::{
    Pipeline,
    StepState,
    ExecutionStatus,
};
use pipeline::execution::{ExecutionEngine, SchedulingStrategy};
use pipeline::agent::{AgentExecutor, AgentError, AgentResponse, ProgressCallback, PiJsonEvent};
use pipeline::core::step::ConditionPattern;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use async_trait::async_trait;

/// Mock agent that returns predefined responses
pub struct MockAgent {
    responses: Arc<Vec<String>>,
    index: Arc<AtomicUsize>,
    simulate_delay: Option<std::time::Duration>,
}

impl MockAgent {
    pub fn new(responses: Vec<String>) -> Self {
        Self {
            responses: Arc::new(responses),
            index: Arc::new(AtomicUsize::new(0)),
            simulate_delay: None,
        }
    }
}

#[async_trait]
impl AgentExecutor for MockAgent {
    async fn execute(&self, _prompt: &str) -> Result<AgentResponse, AgentError> {
        if let Some(delay) = self.simulate_delay {
            tokio::time::sleep(delay).await;
        }

        let idx = self.index.fetch_add(1, Ordering::SeqCst);

        if idx >= self.responses.len() {
            return Err(AgentError::Internal(format!(
                "MockAgent: No response available for request {}",
                idx + 1
            )));
        }

        Ok(AgentResponse::new(self.responses[idx].clone()))
    }

    async fn execute_streaming(
        &self,
        _prompt: &str,
        callback: Option<&dyn ProgressCallback>,
    ) -> Result<AgentResponse, AgentError> {
        // Get the response
        let idx = self.index.fetch_add(1, Ordering::SeqCst);

        if idx >= self.responses.len() {
            return Err(AgentError::Internal(format!(
                "MockAgent: No response available for request {}",
                idx + 1
            )));
        }

        let response = &self.responses[idx];

        // Simulate delay if configured
        if let Some(delay) = self.simulate_delay {
            tokio::time::sleep(delay).await;
        }

        // Generate synthetic events if callback provided
        if let Some(cb) = callback {
            cb.on_event(&PiJsonEvent::AgentStart);

            for ch in response.chars() {
                cb.on_event(&PiJsonEvent::TextDelta {
                    delta: ch.to_string(),
                });
            }

            cb.on_event(&PiJsonEvent::TextEnd {
                content: Some(response.clone()),
            });

            cb.on_event(&PiJsonEvent::AgentEnd);
        }

        Ok(AgentResponse::new(response.clone()))
    }
}

/// Run a pipeline with a mock agent that returns predefined responses
pub async fn run_pipeline_with_mock(
    pipeline: &mut Pipeline,
    responses: Vec<String>,
) -> Result<PipelineTestResult, String> {
    let agent = MockAgent::new(responses);
    run_pipeline_with_agent(pipeline, agent).await
}

/// Run a pipeline with any agent implementation
pub async fn run_pipeline_with_agent<A: AgentExecutor + Send + Sync + 'static>(
    pipeline: &mut Pipeline,
    agent: A,
) -> Result<PipelineTestResult, String> {
    let start = std::time::Instant::now();
    let engine = ExecutionEngine::new(agent, SchedulingStrategy::Sequential);
    engine.execute(pipeline).await?;
    let duration = start.elapsed();

    Ok(PipelineTestResult {
        pipeline: pipeline.clone(),
        duration_ms: duration.as_millis() as u64,
    })
}

/// Test result from running a pipeline
#[derive(Debug, Clone)]
pub struct PipelineTestResult {
    pub pipeline: Pipeline,
    pub duration_ms: u64,
}

impl PipelineTestResult {
    /// Check if the pipeline completed successfully
    pub fn is_success(&self) -> bool {
        matches!(self.pipeline.state.status, ExecutionStatus::Completed)
    }

    /// Check if the pipeline failed
    pub fn is_failed(&self) -> bool {
        matches!(self.pipeline.state.status, ExecutionStatus::Failed)
    }

    /// Get the output of a specific step
    pub fn get_step_output(&self, step_id: &str) -> Option<String> {
        self.pipeline.step(step_id).and_then(|s| {
            match &s.state {
                pipeline::core::state::StepState::Completed { output, .. } => Some(output.clone()),
                _ => None,
            }
        })
    }

    /// Get the state of a specific step
    pub fn get_step_state(&self, step_id: &str) -> Option<&StepState> {
        self.pipeline.step(step_id).map(|s| &s.state)
    }

    /// Get the error message from a failed step
    pub fn get_step_error(&self, step_id: &str) -> Option<String> {
        self.pipeline.step(step_id).and_then(|s| {
            match &s.state {
                pipeline::core::state::StepState::Failed { error, .. } => Some(error.clone()),
                _ => None,
            }
        })
    }

    /// Count how many times a step was executed (based on attempt count)
    pub fn count_step_attempts(&self, step_id: &str) -> usize {
        self.pipeline.step(step_id)
            .and_then(|s| match &s.state {
                StepState::Completed { attempts, .. } => Some(*attempts),
                StepState::Failed { attempts, .. } => Some(*attempts),
                _ => Some(0),
            })
            .unwrap_or(0)
    }

    /// Get the execution order of steps
    pub fn execution_order(&self) -> Vec<String> {
        self.pipeline.execution_order().to_vec()
    }

    /// Get completed steps in order
    pub fn completed_steps(&self) -> Vec<String> {
        self.pipeline.steps.values()
            .filter(|s| matches!(s.state, StepState::Completed { .. }))
            .map(|s| s.id.clone())
            .collect()
    }

    /// Get failed steps
    pub fn failed_steps(&self) -> Vec<String> {
        self.pipeline.steps.values()
            .filter(|s| matches!(s.state, StepState::Failed { .. }))
            .map(|s| s.id.clone())
            .collect()
    }

    /// Get a summary of the result
    pub fn summary(&self) -> String {
        let status = match self.pipeline.state.status {
            ExecutionStatus::Completed => "✅ Completed",
            ExecutionStatus::Failed => "❌ Failed",
            ExecutionStatus::Running => "🔄 Running",
            _ => "❓ Unknown",
        };
        format!(
            "{} - {} steps completed, {} steps failed, {}ms",
            status,
            self.completed_steps().len(),
            self.failed_steps().len(),
            self.duration_ms
        )
    }
}

/// Assert a step was executed and check its output
pub fn assert_step_executed(
    result: &PipelineTestResult,
    step_id: &str,
    expected_output: &str,
) {
    let step = result
        .pipeline
        .step(step_id)
        .unwrap_or_else(|| panic!("Step '{}' not found in result", step_id));

    assert!(
        matches!(step.state, StepState::Completed { .. }),
        "Step '{}' should be completed, but was in state: {:?}",
        step_id, step.state
    );

    let output = match &step.state {
        StepState::Completed { output, .. } => output.clone(),
        _ => panic!("Step '{}' is not completed", step_id),
    };

    assert!(
        output.contains(expected_output),
        "Step '{}' output:\n{}\n\ndoes not contain:\n{}",
        step_id, output, expected_output
    );
}

/// Assert a step failed with specific message
pub fn assert_step_failed(
    result: &PipelineTestResult,
    step_id: &str,
    expected_error: &str,
) {
    let step = result
        .pipeline
        .step(step_id)
        .unwrap_or_else(|| panic!("Step '{}' not found in result", step_id));

    assert!(
        matches!(step.state, StepState::Failed { .. }),
        "Step '{}' should have failed, but was in state: {:?}",
        step_id, step.state
    );

    let error = match &step.state {
        StepState::Failed { error, .. } => error.clone(),
        _ => panic!("Step '{}' is not failed", step_id),
    };

    assert!(
        error.contains(expected_error),
        "Step '{}' error:\n{}\n\ndoes not contain:\n{}",
        step_id, error, expected_error
    );
}

/// Assert pipeline completed successfully
pub fn assert_pipeline_completed(result: &PipelineTestResult) {
    assert!(
        result.is_success(),
        "Pipeline should be completed, but was: {}",
        result.summary()
    );
}

/// Assert pipeline failed
pub fn assert_pipeline_failed(result: &PipelineTestResult) {
    assert!(
        result.is_failed(),
        "Pipeline should have failed, but was: {}",
        result.summary()
    );
}

/// Assert specific steps were executed in order
pub fn assert_execution_order(
    result: &PipelineTestResult,
    expected_order: &[&str],
) {
    // Get completed or failed steps in the order they appear in execution_order
    let actual_order: Vec<String> = result.pipeline.execution_order()
        .iter()
        .filter(|id| {
            result.pipeline.step(id)
                .map(|s| {
                    matches!(s.state, StepState::Completed { .. } | StepState::Failed { .. })
                })
                .unwrap_or(false)
        })
        .cloned()
        .collect();

    assert_eq!(
        actual_order, expected_order,
        "Expected execution order: {:?}\nActual: {:?}",
        expected_order, actual_order
    );
}

/// Count how many times a step was executed
pub fn count_step_executions(result: &PipelineTestResult, step_id: &str) -> usize {
    result.count_step_attempts(step_id)
}

/// Create a continuation condition for retry
pub fn retry_continuation(pattern: &str) -> pipeline::core::step::ContinuationCondition {
    pipeline::core::step::ContinuationCondition {
        pattern: ConditionPattern::Simple(pattern.to_string()),
        action: pipeline::core::config::ContinuationAction::Retry,
        target: None,
    }
}

/// Create a continuation condition for routing
pub fn route_continuation(pattern: &str, target: &str) -> pipeline::core::step::ContinuationCondition {
    pipeline::core::step::ContinuationCondition {
        pattern: ConditionPattern::Simple(pattern.to_string()),
        action: pipeline::core::config::ContinuationAction::Route,
        target: Some(target.to_string()),
    }
}

/// Parse a pipeline from YAML string
pub fn pipeline_from_yaml(yaml: &str) -> Pipeline {
    let config = pipeline::core::config::PipelineConfig::from_yaml(yaml)
        .unwrap_or_else(|e| panic!("Failed to parse pipeline YAML: {}", e));
    config.to_pipeline()
}

/// Create a minimal pipeline for testing
pub fn minimal_pipeline() -> Pipeline {
    let yaml = r#"
name: "Test Pipeline"
steps:
  - id: "step1"
    name: "Step 1"
    prompt: "Do something"
    termination:
      success_pattern: "DONE"
"#;
    pipeline_from_yaml(yaml)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_run_pipeline_with_mock_simple() {
        let mut pipeline = minimal_pipeline();
        let responses = vec!["Working... DONE".to_string()];

        let result = run_pipeline_with_mock(&mut pipeline, responses).await;

        assert!(result.is_ok());
        let test_result = result.unwrap();
        assert!(test_result.is_success());
        assert_step_executed(&test_result, "step1", "DONE");
    }

    #[test]
    fn test_assert_step_executed() {
        let mut pipeline = minimal_pipeline();
        if let Some(step) = pipeline.step_mut("step1") {
            step.state = StepState::Completed {
                output: "Task completed with DONE marker".to_string(),
                attempts: 1,
                started_at: chrono::Utc::now(),
                completed_at: chrono::Utc::now(),
            };
        }

        let result = PipelineTestResult {
            pipeline,
            duration_ms: 100,
        };

        assert_step_executed(&result, "step1", "DONE");
    }

    #[test]
    fn test_assert_step_failed() {
        let mut pipeline = minimal_pipeline();
        if let Some(step) = pipeline.step_mut("step1") {
            step.state = StepState::Failed {
                error: "Max retries exceeded".to_string(),
                attempts: 3,
                last_started_at: chrono::Utc::now(),
                failed_at: chrono::Utc::now(),
            };
        }

        let result = PipelineTestResult {
            pipeline,
            duration_ms: 100,
        };

        assert_step_failed(&result, "step1", "exceeded");
    }

    #[test]
    fn test_assert_execution_order() {
        let yaml = r#"
name: "Test Pipeline"
steps:
  - id: "a"
    name: "A"
    prompt: "Test"
    termination:
      success_pattern: "DONE"
  - id: "b"
    name: "B"
    prompt: "Test"
    depends_on: ["a"]
    termination:
      success_pattern: "DONE"
  - id: "c"
    name: "C"
    prompt: "Test"
    depends_on: ["b"]
    termination:
      success_pattern: "DONE"
"#;
        let mut pipeline = pipeline_from_yaml(yaml);

        // Mark all steps as completed
        let now = chrono::Utc::now();
        for step_id in ["a", "b", "c"] {
            if let Some(step) = pipeline.step_mut(step_id) {
                step.state = StepState::Completed {
                    output: "DONE".to_string(),
                    attempts: 1,
                    started_at: now,
                    completed_at: now,
                };
            }
        }

        let result = PipelineTestResult {
            pipeline,
            duration_ms: 100,
        };

        assert_execution_order(&result, &["a", "b", "c"]);
    }
}
