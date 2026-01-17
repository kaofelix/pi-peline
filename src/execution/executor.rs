//! Step executor - runs individual steps with the agent

use crate::{
    agent::{AgentExecutor, ProgressCallback},
    core::{Step, PipelineContext},
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::time::{timeout, Duration};
use tracing::{debug, info, warn, error};

/// Result of executing a step
#[derive(Debug, Clone)]
pub enum ExecutionResult {
    /// Step completed successfully
    Success {
        output: String,
        next_step: Option<String>,
    },
    /// Step needs continuation (retry or route to another step)
    Continue {
        action: ContinueAction,
        target: Option<String>,
    },
    /// Step failed but should route to a failure handler
    FailedWithRoute {
        error: String,
        next_step: String,
    },
    /// Step failed with no handler
    Failed {
        error: String,
    },
    /// Execution was interrupted (Phase 4)
    Interrupted {
        step_id: String,
        accumulated_output: String,
        recent_lines: Vec<String>,
        original_prompt: String,
    },
}

/// Action to take for continuation
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContinueAction {
    /// Retry the same step
    Retry,
    /// Route to a different step
    Route(String),
}

/// Executes a single step
pub struct StepExecutor<A> {
    agent: A,
}

impl<A: AgentExecutor> StepExecutor<A> {
    pub fn new(agent: A) -> Self {
        Self { agent }
    }

    /// Execute a step and return the result
    pub async fn execute(
        &self,
        step: &Step,
        context: &PipelineContext,
        callback: Option<&dyn ProgressCallback>,
    ) -> ExecutionResult {
        info!("Executing step: {}", step.id);

        let effective_prompt = step.build_effective_prompt(&context.get_rendering_variables());
        debug!("Effective prompt for step {}: {}", step.id, effective_prompt);

        // Execute with streaming for live output display
        let timeout_duration = Duration::from_secs(step.timeout_secs);
        let result = match timeout(
            timeout_duration,
            self.agent.execute_streaming(&effective_prompt, callback)
        ).await {
            Ok(Ok(response)) => response,
            Ok(Err(e)) => {
                error!("Agent error for step {}: {}", step.id, e);
                return ExecutionResult::Failed {
                    error: e.to_string(),
                };
            }
            Err(_) => {
                error!("Timeout for step {} after {}s", step.id, step.timeout_secs);
                return ExecutionResult::Failed {
                    error: format!("Timeout after {} seconds", step.timeout_secs),
                };
            }
        };

        debug!("Agent response for step {}: {}", step.id, result.content);

        // Check for continuation first (agent wants more work)
        if step.needs_continuation(&result.content) {
            if let Some((action, target)) = step.get_continuation_action() {
                return match action {
                    crate::core::config::ContinuationAction::Retry => {
                        info!("Step {} requested continuation (retry)", step.id);
                        ExecutionResult::Continue {
                            action: ContinueAction::Retry,
                            target: None,
                        }
                    }
                    crate::core::config::ContinuationAction::Route => {
                        let target_id = target.unwrap().clone();
                        info!("Step {} requested continuation (route to {})", step.id, target_id);
                        ExecutionResult::Continue {
                            action: ContinueAction::Route(target_id.clone()),
                            target: Some(target_id),
                        }
                    }
                };
            }
        }

        // Check for successful completion
        if step.is_success(&result.content) {
            let next_step = step.next_step_on_success().cloned();
            info!("Step {} completed successfully", step.id);
            if let Some(ref next) = next_step {
                info!("  Next step: {}", next);
            }
            return ExecutionResult::Success {
                output: result.content,
                next_step,
            };
        }

        // No expected pattern found - handle as retry (if no on_failure route)
        warn!(
            "Step {} did not output expected termination pattern",
            step.id
        );
        let next_step = step.next_step_on_failure().cloned();

        // If there's an on_failure route, route to failure handler
        if let Some(target) = next_step {
            info!("Step {} routing to failure handler: {}", step.id, target);
            ExecutionResult::FailedWithRoute {
                error: "No termination pattern found".to_string(),
                next_step: target,
            }
        } else {
            // No failure handler - retry the step instead of failing the pipeline
            info!("Step {} will retry for termination pattern", step.id);
            ExecutionResult::Continue {
                action: ContinueAction::Retry,
                target: None,
            }
        }
    }

    /// Execute a step with interruption support
    ///
    /// This method checks the `interrupted` flag before and after execution.
    /// If the flag is set at any point, it returns an `Interrupted` result
    /// with the accumulated output and context.
    ///
    /// # Arguments
    ///
    /// * `step` - The step to execute
    /// * `context` - The pipeline context
    /// * `callback` - Optional progress callback for streaming
    /// * `interrupted` - Atomic boolean flag for interruption detection
    ///
    /// # Returns
    ///
    /// An `ExecutionResult` that may be:
    /// - `Success` - Step completed normally
    /// - `Failed` - Step failed with an error
    /// - `Interrupted` - Execution was interrupted by user
    pub async fn execute_interruptible(
        &self,
        step: &Step,
        context: &PipelineContext,
        callback: Option<&dyn ProgressCallback>,
        interrupted: Arc<AtomicBool>,
    ) -> ExecutionResult {
        info!("Executing step with interruption support: {}", step.id);

        // Check for interruption before starting
        if interrupted.load(Ordering::SeqCst) {
            info!("Step {} interrupted before execution", step.id);
            return ExecutionResult::Interrupted {
                step_id: step.id.clone(),
                accumulated_output: String::new(),
                recent_lines: vec![],
                original_prompt: step.prompt_template.clone(),
            };
        }

        let effective_prompt = step.build_effective_prompt(&context.get_rendering_variables());
        debug!("Effective prompt for step {}: {}", step.id, effective_prompt);

        // Execute with streaming for live output display
        let timeout_duration = Duration::from_secs(step.timeout_secs);
        let result = match timeout(
            timeout_duration,
            self.agent.execute_streaming(&effective_prompt, callback)
        ).await {
            Ok(Ok(response)) => response,
            Ok(Err(e)) => {
                error!("Agent error for step {}: {}", step.id, e);
                // Check for interruption
                if interrupted.load(Ordering::SeqCst) {
                    return ExecutionResult::Interrupted {
                        step_id: step.id.clone(),
                        accumulated_output: String::new(),
                        recent_lines: callback
                            .and_then(|cb| cb.get_context_lines())
                            .unwrap_or_default(),
                        original_prompt: effective_prompt,
                    };
                }
                return ExecutionResult::Failed {
                    error: e.to_string(),
                };
            }
            Err(_) => {
                error!("Timeout for step {} after {}s", step.id, step.timeout_secs);
                // Check for interruption
                if interrupted.load(Ordering::SeqCst) {
                    return ExecutionResult::Interrupted {
                        step_id: step.id.clone(),
                        accumulated_output: String::new(),
                        recent_lines: callback
                            .and_then(|cb| cb.get_context_lines())
                            .unwrap_or_default(),
                        original_prompt: effective_prompt,
                    };
                }
                return ExecutionResult::Failed {
                    error: format!("Timeout after {} seconds", step.timeout_secs),
                };
            }
        };

        // Check for interruption after execution
        if interrupted.load(Ordering::SeqCst) {
            info!("Step {} interrupted after execution", step.id);
            return ExecutionResult::Interrupted {
                step_id: step.id.clone(),
                accumulated_output: result.content,
                recent_lines: callback
                    .and_then(|cb| cb.get_context_lines())
                    .unwrap_or_default(),
                original_prompt: effective_prompt,
            };
        }

        debug!("Agent response for step {}: {}", step.id, result.content);

        // Check for continuation first (agent wants more work)
        if step.needs_continuation(&result.content) {
            if let Some((action, target)) = step.get_continuation_action() {
                return match action {
                    crate::core::config::ContinuationAction::Retry => {
                        info!("Step {} requested continuation (retry)", step.id);
                        ExecutionResult::Continue {
                            action: ContinueAction::Retry,
                            target: None,
                        }
                    }
                    crate::core::config::ContinuationAction::Route => {
                        let target_id = target.unwrap().clone();
                        info!("Step {} requested continuation (route to {})", step.id, target_id);
                        ExecutionResult::Continue {
                            action: ContinueAction::Route(target_id.clone()),
                            target: Some(target_id),
                        }
                    }
                };
            }
        }

        // Check for successful completion
        if step.is_success(&result.content) {
            let next_step = step.next_step_on_success().cloned();
            info!("Step {} completed successfully", step.id);
            if let Some(ref next) = next_step {
                info!("  Next step: {}", next);
            }
            return ExecutionResult::Success {
                output: result.content,
                next_step,
            };
        }

        // No expected pattern found - handle as retry (if no on_failure route)
        warn!(
            "Step {} did not output expected termination pattern",
            step.id
        );
        let next_step = step.next_step_on_failure().cloned();

        // If there's an on_failure route, route to failure handler
        if let Some(target) = next_step {
            info!("Step {} routing to failure handler: {}", step.id, target);
            ExecutionResult::FailedWithRoute {
                error: "No termination pattern found".to_string(),
                next_step: target,
            }
        } else {
            // No failure handler - retry the step instead of failing the pipeline
            info!("Step {} will retry for termination pattern", step.id);
            ExecutionResult::Continue {
                action: ContinueAction::Retry,
                target: None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::step::{Step, ContinuationCondition};
    use crate::core::condition::TerminationCondition as DomainTerminationCondition;
    use crate::core::state::StepState;
    use crate::agent::AgentResponse;

    // Mock agent executor for testing
    struct MockAgent {
        response: String,
    }

    #[async_trait::async_trait]
    impl AgentExecutor for MockAgent {
        async fn execute(&self, _prompt: &str) -> Result<AgentResponse, crate::agent::AgentError> {
            Ok(AgentResponse::new(self.response.clone()))
        }

        async fn execute_streaming(
            &self,
            _prompt: &str,
            _callback: Option<&dyn crate::agent::ProgressCallback>,
        ) -> Result<AgentResponse, crate::agent::AgentError> {
            // For testing, just return the same response
            Ok(AgentResponse::new(self.response.clone()))
        }
    }

    #[tokio::test]
    async fn test_step_success() {
        let step = Step {
            id: "test".to_string(),
            prompt_template: "Do the task".to_string(),
            dependencies: vec![],
            termination: Some(DomainTerminationCondition {
                success_pattern: crate::core::step::ConditionPattern::Simple("DONE".to_string()),
                on_success: Some("next".to_string()),
                on_failure: None,
            }),
            continuation: None,
            max_retries: 3,
            timeout_secs: 300,
            state: StepState::Pending,
        };

        let agent = MockAgent {
            response: "Working... DONE".to_string(),
        };
        let executor = StepExecutor::new(agent);
        let context = PipelineContext::new();

        let result = executor.execute(&step, &context, None).await;

        match result {
            ExecutionResult::Success { output, next_step } => {
                assert_eq!(output, "Working... DONE");
                assert_eq!(next_step, Some("next".to_string()));
            }
            _ => panic!("Expected success, got {:?}", result),
        }
    }

    #[tokio::test]
    async fn test_step_continuation_retry() {
        let step = Step {
            id: "test".to_string(),
            prompt_template: "Do the task".to_string(),
            dependencies: vec![],
            termination: Some(DomainTerminationCondition {
                success_pattern: crate::core::step::ConditionPattern::Simple("DONE".to_string()),
                on_success: None,
                on_failure: None,
            }),
            continuation: Some(ContinuationCondition {
                pattern: crate::core::step::ConditionPattern::Simple("CONTINUE".to_string()),
                action: crate::core::config::ContinuationAction::Retry,
                target: None,
            }),
            max_retries: 3,
            timeout_secs: 300,
            state: StepState::Pending,
        };

        let agent = MockAgent {
            response: "Need more work... CONTINUE".to_string(),
        };
        let executor = StepExecutor::new(agent);
        let context = PipelineContext::new();

        let result = executor.execute(&step, &context, None).await;

        match result {
            ExecutionResult::Continue { action, target } => {
                assert_eq!(action, ContinueAction::Retry);
                assert!(target.is_none());
            }
            _ => panic!("Expected continue, got {:?}", result),
        }
    }

    // Phase 4: Interruption Result Tests

    #[test]
    fn test_execution_result_interrupted_variant_exists() {
        // Test that Interrupted variant can be constructed
        let result = ExecutionResult::Interrupted {
            step_id: "test-step".to_string(),
            accumulated_output: "partial output".to_string(),
            recent_lines: vec!["line 1".to_string(), "line 2".to_string()],
            original_prompt: "test prompt".to_string(),
        };

        assert!(matches!(result, ExecutionResult::Interrupted { .. }));
    }

    #[test]
    fn test_execution_result_interrupted_fields() {
        let result = ExecutionResult::Interrupted {
            step_id: "test-step".to_string(),
            accumulated_output: "partial output".to_string(),
            recent_lines: vec!["line 1".to_string()],
            original_prompt: "test prompt".to_string(),
        };

        if let ExecutionResult::Interrupted {
            step_id,
            accumulated_output,
            recent_lines,
            original_prompt,
        } = result
        {
            assert_eq!(step_id, "test-step");
            assert_eq!(accumulated_output, "partial output");
            assert_eq!(recent_lines.len(), 1);
            assert_eq!(recent_lines[0], "line 1");
            assert_eq!(original_prompt, "test prompt");
        } else {
            panic!("Expected Interrupted variant");
        }
    }

    // Phase 4: Executor Interruption Tests

    #[tokio::test]
    async fn test_executor_signal_handling_sets_interrupted_flag() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;

        let interrupted_flag = Arc::new(AtomicBool::new(false));
        let step = Step {
            id: "test".to_string(),
            prompt_template: "Do the task".to_string(),
            dependencies: vec![],
            termination: Some(DomainTerminationCondition {
                success_pattern: crate::core::step::ConditionPattern::Simple("DONE".to_string()),
                on_success: None,
                on_failure: None,
            }),
            continuation: None,
            max_retries: 3,
            timeout_secs: 300,
            state: StepState::Pending,
        };

        let agent = MockAgent {
            response: "Working... DONE".to_string(),
        };
        let executor = StepExecutor::new(agent);
        let context = PipelineContext::new();

        // Set the interrupted flag before execution
        interrupted_flag.store(true, Ordering::SeqCst);

        // Execute with interruptible wrapper
        let result = executor.execute_interruptible(&step, &context, None, interrupted_flag).await;

        // Verify that the executor detected the interruption
        assert!(matches!(result, ExecutionResult::Interrupted { .. }));
    }

    #[tokio::test]
    async fn test_executor_returns_interrupted_result_when_flag_set() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;

        let interrupted_flag = Arc::new(AtomicBool::new(false));
        let step = Step {
            id: "test".to_string(),
            prompt_template: "Do the task".to_string(),
            dependencies: vec![],
            termination: Some(DomainTerminationCondition {
                success_pattern: crate::core::step::ConditionPattern::Simple("DONE".to_string()),
                on_success: None,
                on_failure: None,
            }),
            continuation: None,
            max_retries: 3,
            timeout_secs: 300,
            state: StepState::Pending,
        };

        let agent = MockAgent {
            response: "Working... DONE".to_string(),
        };
        let executor = StepExecutor::new(agent);
        let context = PipelineContext::new();

        // Set the interrupted flag before execution
        interrupted_flag.store(true, Ordering::SeqCst);

        let result = executor.execute_interruptible(&step, &context, None, interrupted_flag).await;

        match result {
            ExecutionResult::Interrupted { step_id, .. } => {
                assert_eq!(step_id, "test");
            }
            _ => panic!("Expected Interrupted result, got {:?}", result),
        }
    }

    #[tokio::test]
    async fn test_step_no_termination_pattern_retries() {
        // When no termination pattern is found and there's no on_failure handler,
        // the step should retry instead of failing the pipeline
        let step = Step {
            id: "test".to_string(),
            prompt_template: "Do the task".to_string(),
            dependencies: vec![],
            termination: Some(DomainTerminationCondition {
                success_pattern: crate::core::step::ConditionPattern::Simple("DONE".to_string()),
                on_success: None,
                on_failure: None,  // No failure handler
            }),
            continuation: None,
            max_retries: 3,
            timeout_secs: 300,
            state: StepState::Pending,
        };

        let agent = MockAgent {
            response: "Working on it...".to_string(),  // No "DONE" pattern
        };
        let executor = StepExecutor::new(agent);
        let context = PipelineContext::new();

        let result = executor.execute(&step, &context, None).await;

        // Should return Continue with Retry action, not Failed
        match result {
            ExecutionResult::Continue { action, target } => {
                assert_eq!(action, ContinueAction::Retry);
                assert!(target.is_none());
            }
            _ => panic!("Expected Continue with Retry, got {:?}", result),
        }
    }

    #[tokio::test]
    async fn test_step_no_termination_pattern_with_failure_handler_routes() {
        // When no termination pattern is found but there's an on_failure handler,
        // the step should route to the failure handler
        let step = Step {
            id: "test".to_string(),
            prompt_template: "Do the task".to_string(),
            dependencies: vec![],
            termination: Some(DomainTerminationCondition {
                success_pattern: crate::core::step::ConditionPattern::Simple("DONE".to_string()),
                on_success: None,
                on_failure: Some("handler".to_string()),  // Has failure handler
            }),
            continuation: None,
            max_retries: 3,
            timeout_secs: 300,
            state: StepState::Pending,
        };

        let agent = MockAgent {
            response: "Working on it...".to_string(),  // No "DONE" pattern
        };
        let executor = StepExecutor::new(agent);
        let context = PipelineContext::new();

        let result = executor.execute(&step, &context, None).await;

        // Should return FailedWithRoute
        match result {
            ExecutionResult::FailedWithRoute { error, next_step } => {
                assert_eq!(error, "No termination pattern found");
                assert_eq!(next_step, "handler");
            }
            _ => panic!("Expected FailedWithRoute, got {:?}", result),
        }
    }
}

