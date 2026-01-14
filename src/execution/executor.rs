//! Step executor - runs individual steps with the agent

use crate::{
    agent::{AgentExecutor, AgentError},
    core::{Step, StepState, PipelineContext},
};
use chrono::Utc;
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
    /// Step failed
    Failed {
        error: String,
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
    ) -> ExecutionResult {
        info!("Executing step: {}", step.id);

        let effective_prompt = step.build_effective_prompt(&context.get_rendering_variables());
        debug!("Effective prompt for step {}: {}", step.id, effective_prompt);

        // Execute with timeout
        let timeout_duration = Duration::from_secs(step.timeout_secs);
        let result = match timeout(timeout_duration, self.agent.execute(&effective_prompt)).await {
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

        // No expected pattern found - handle as failure
        warn!(
            "Step {} did not output expected termination pattern",
            step.id
        );
        let next_step = step.next_step_on_failure().cloned();

        // If there's an on_failure route, treat as "success" with routing
        if next_step.is_some() {
            info!("Step {} routing to failure handler: {:?}", step.id, next_step);
            ExecutionResult::Success {
                output: result.content,
                next_step,
            }
        } else {
            error!("Step {} failed with no handler", step.id);
            ExecutionResult::Failed {
                error: "No termination pattern found".to_string(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::{ContinuationConfig, TerminationConfig};
    use crate::core::step::{Step, StepDefaults, ContinuationCondition};
    use crate::core::condition::TerminationCondition as DomainTerminationCondition;
    use crate::agent::AgentResponse;
    use std::collections::HashMap;

    // Mock agent executor for testing
    struct MockAgent {
        response: String,
    }

    #[async_trait::async_trait]
    impl AgentExecutor for MockAgent {
        async fn execute(&self, _prompt: &str) -> Result<AgentResponse, crate::agent::AgentError> {
            Ok(AgentResponse::new(self.response.clone()))
        }
    }

    #[tokio::test]
    async fn test_step_success() {
        let step = Step {
            id: "test".to_string(),
            name: "Test".to_string(),
            description: None,
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
            allow_parallel: false,
            state: StepState::Pending,
        };

        let agent = MockAgent {
            response: "Working... DONE".to_string(),
        };
        let executor = StepExecutor::new(agent);
        let context = PipelineContext::new();

        let result = executor.execute(&step, &context).await;

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
            name: "Test".to_string(),
            description: None,
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
                carry_notes: false,
            }),
            max_retries: 3,
            timeout_secs: 300,
            allow_parallel: false,
            state: StepState::Pending,
        };

        let agent = MockAgent {
            response: "Need more work... CONTINUE".to_string(),
        };
        let executor = StepExecutor::new(agent);
        let context = PipelineContext::new();

        let result = executor.execute(&step, &context).await;

        match result {
            ExecutionResult::Continue { action, target } => {
                assert_eq!(action, ContinueAction::Retry);
                assert!(target.is_none());
            }
            _ => panic!("Expected continue, got {:?}", result),
        }
    }
}
