//! Main execution engine - orchestrates the entire pipeline run

use crate::{
    core::{Pipeline, StepState, ExecutionStatus},
    execution::{StepExecutor, ExecutionResult, ContinueAction, ExecutionScheduler, SchedulingStrategy},
    agent::AgentExecutor,
};
use tokio::sync::Mutex;
use tracing::{info, warn, error};
use std::sync::Arc;
use uuid::Uuid;

/// Events that can occur during pipeline execution
#[derive(Debug, Clone)]
pub enum ExecutionEvent {
    PipelineStarted {
        execution_id: Uuid,
        pipeline_name: String,
    },
    StepStarted {
        step_id: String,
        attempt: usize,
    },
    StepOutput {
        step_id: String,
        output: String,
    },
    StepCompleted {
        step_id: String,
        next_step: Option<String>,
    },
    StepFailed {
        step_id: String,
        error: String,
    },
    StepContinued {
        step_id: String,
        action: ContinueAction,
    },
    StepRetrying {
        step_id: String,
        attempt: usize,
        max_retries: usize,
    },
    StepRerouted {
        from_step: String,
        to_step: String,
    },
    PipelineCompleted {
        execution_id: Uuid,
        status: ExecutionStatus,
    },
}

/// Type for event handlers
pub type EventHandler = Arc<dyn Fn(ExecutionEvent) + Send + Sync>;

/// Main pipeline execution engine
pub struct ExecutionEngine<A> {
    scheduler: Arc<Mutex<ExecutionScheduler>>,
    executor: Arc<StepExecutor<A>>,
    event_handlers: Arc<Mutex<Vec<EventHandler>>>,
}

impl<A: AgentExecutor + Send + Sync + 'static> ExecutionEngine<A> {
    pub fn new(
        agent: A,
        strategy: SchedulingStrategy,
    ) -> Self {
        let executor = Arc::new(StepExecutor::new(agent));
        let scheduler = Arc::new(Mutex::new(ExecutionScheduler::new(strategy)));

        Self {
            scheduler,
            executor,
            event_handlers: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Add an event handler
    pub fn add_event_handler<F>(&self, handler: F)
    where
        F: Fn(ExecutionEvent) + Send + Sync + 'static,
    {
        let handlers = self.event_handlers.clone();
        tokio::spawn(async move {
            handlers.lock().await.push(Arc::new(handler));
        });
    }

    /// Emit an event to all handlers
    async fn emit_event(&self, event: ExecutionEvent) {
        let handlers = self.event_handlers.lock().await;
        for handler in handlers.iter() {
            handler(event.clone());
        }
    }

    /// Execute the entire pipeline
    pub async fn execute(&self, pipeline: &mut Pipeline) -> Result<(), String> {
        let execution_id = pipeline.state.execution_id;
        let pipeline_name = pipeline.name.clone();
        let pipeline_name_ref = pipeline_name.as_str();

        info!("Starting pipeline execution: {} ({})", pipeline_name_ref, execution_id);
        self.emit_event(ExecutionEvent::PipelineStarted {
            execution_id,
            pipeline_name: pipeline_name.clone(),
        })
        .await;

        pipeline.state.start(pipeline.steps.len());

        // Main execution loop
        while !pipeline.is_complete() && !pipeline.has_failed() {
            // Get next steps to run
            let step_ids = {
                let scheduler = self.scheduler.lock().await;
                scheduler.next_steps(pipeline)
            };

            if step_ids.is_empty() {
                // Check if we're stuck (running but no progress)
                if pipeline.running_steps().is_empty() {
                    // Check if all steps are in terminal states
                    let all_terminal = pipeline.steps.values()
                        .all(|s| s.state.is_terminal());

                    // Check if remaining steps can't run due to failed dependencies
                    let blocked_by_failed_deps = pipeline.steps.values()
                        .any(|s| {
                            matches!(s.state, StepState::Pending)
                                && s.dependencies.iter().any(|dep| {
                                    pipeline.step(dep).is_some_and(|dep_step| {
                                        matches!(dep_step.state, StepState::Failed { .. })
                                    })
                                })
                        });

                    // Check if there are retrying steps with unsatisfied dependencies
                    let retrying_blocked = pipeline.steps.values()
                        .any(|s| {
                            matches!(s.state, StepState::Retrying { .. })
                                && s.dependencies.iter().any(|dep| {
                                    pipeline.step(dep).is_some_and(|dep_step| {
                                        !matches!(dep_step.state, StepState::Completed { .. } | StepState::Failed { .. })
                                    })
                                })
                        });

                    if all_terminal || (blocked_by_failed_deps && !retrying_blocked) {
                        // Pipeline is complete
                        let status = if pipeline.has_failed() {
                            ExecutionStatus::Failed
                        } else {
                            ExecutionStatus::Completed
                        };
                        if matches!(pipeline.state.status, ExecutionStatus::Running) {
                            pipeline.state.status = status;
                        }
                        self.emit_event(ExecutionEvent::PipelineCompleted {
                            execution_id,
                            status,
                        })
                        .await;
                        return Ok(());
                    }

                    // Otherwise, truly stuck
                    error!("No steps ready to run and none running - pipeline stuck");
                    pipeline.state.fail();
                    self.emit_event(ExecutionEvent::PipelineCompleted {
                        execution_id,
                        status: ExecutionStatus::Failed,
                    })
                    .await;
                    return Err("Pipeline stuck - no runnable steps".to_string());
                }

                // Wait a bit before checking again
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                continue;
            }

            // Execute each ready step
            for step_id in &step_ids {
                self.execute_step(pipeline, step_id).await?;
            }

            // Check for completion - if no runnable steps but some completed, we're done
            if step_ids.is_empty()
                && !pipeline.running_steps().is_empty()
                && pipeline.state.completed_steps > 0
            {
                // Check if all steps are in terminal states
                let all_terminal = pipeline.steps.values()
                    .all(|s| s.state.is_terminal());

                if all_terminal {
                    info!("All steps in terminal states, pipeline complete");
                    break;
                }
            }

            // Update state counts
            self.update_state_counts(pipeline);
        }

        // Pipeline is complete
        let status = if pipeline.has_failed() {
            pipeline.state.fail();
            ExecutionStatus::Failed
        } else {
            pipeline.state.complete();
            ExecutionStatus::Completed
        };

        info!(
            "Pipeline execution finished: {} - {:?}",
            pipeline_name_ref, status
        );
        self.emit_event(ExecutionEvent::PipelineCompleted {
            execution_id,
            status,
        })
        .await;

        Ok(())
    }

    /// Execute a single step
    async fn execute_step(&self, pipeline: &mut Pipeline, step_id: &str) -> Result<(), String> {
        let step = match pipeline.step(step_id).cloned() {
            Some(s) => s,
            None => return Err(format!("Step {} not found", step_id)),
        };

        // Get or create step state with attempt tracking
        let (attempt, is_retry) = match &step.state {
            StepState::Pending => (1, false),
            StepState::Retrying { attempt: prev_attempt } => {
                // For Retrying state, the attempt is already correct (we didn't increment when routing)
                (*prev_attempt, true)
            }
            StepState::Running { attempt, .. } => (*attempt + 1, true),
            _ => return Err(format!("Step {} in invalid state for execution", step_id)),
        };

        // Check retry limit (retry_count > max_retries means we've exceeded retries)
        // retry_count = attempt - 1 (since attempt 1 is not a retry)
        // max_retries=0 means only initial attempt (no retries)
        // max_retries=3 means initial + 3 retries = 4 total attempts
        let max_retries = step.max_retries;
        let retry_count = if is_retry { attempt - 1 } else { 0 };
        if retry_count > max_retries {
            warn!("Step {} exceeded retry limit (retry_count {} > max_retries {})", step_id, retry_count, max_retries);
            let error = format!("Exceeded retry limit of {}", max_retries);
            // Use attempt - 1 as the actual number of completed attempts
            self.mark_step_failed(pipeline, step_id, error, attempt - 1).await;
            return Ok(());
        }

        // Update step state to running
        if let Some(s) = pipeline.step_mut(step_id) {
            s.state = StepState::Running {
                started_at: chrono::Utc::now(),
                attempt,
            };
        }

        self.emit_event(ExecutionEvent::StepStarted {
            step_id: step_id.to_string(),
            attempt,
        })
        .await;

        if is_retry {
            self.emit_event(ExecutionEvent::StepRetrying {
                step_id: step_id.to_string(),
                attempt,
                max_retries,
            })
            .await;
        }

        // Create context and execute
        let context = pipeline.create_context_for_step(step_id);
        let result = self.executor.execute(&step, &context).await;

        match result {
            ExecutionResult::Success { output, next_step } => {
                self.mark_step_success(pipeline, step_id, output).await;

                // Enqueue next step if specified
                if let Some(next) = next_step.clone() {
                    // Reset target step to Retrying if it was already completed
                    // Increment attempts to track re-execution due to routing
                    let next_attempt = match pipeline.step(&next) {
                        Some(step) => match &step.state {
                            StepState::Completed { attempts, .. } | StepState::Failed { attempts, .. } => {
                                *attempts + 1
                            }
                            StepState::Pending | StepState::Retrying { .. } => 1,
                            _ => 1,
                        },
                        None => 1,
                    };

                    if let Some(step) = pipeline.step_mut(&next) {
                        if matches!(step.state, StepState::Completed { .. } | StepState::Failed { .. }) {
                            step.state = StepState::Retrying {
                                attempt: next_attempt,
                            };
                        }
                    }

                    let mut scheduler = self.scheduler.lock().await;
                    scheduler.enqueue(next.clone());
                    self.emit_event(ExecutionEvent::StepCompleted {
                        step_id: step_id.to_string(),
                        next_step: Some(next),
                    })
                    .await;
                } else {
                    self.emit_event(ExecutionEvent::StepCompleted {
                        step_id: step_id.to_string(),
                        next_step: None,
                    })
                    .await;
                }
            }
            ExecutionResult::Continue { action, target } => {
                self.handle_continuation(pipeline, step_id, action, target).await?;
            }
            ExecutionResult::FailedWithRoute { error, next_step } => {
                // Mark step as failed but route to handler
                self.mark_step_failed(pipeline, step_id, error, attempt).await;

                // Don't fail the pipeline yet - route to handler first
                // Reset pipeline state to running (so it doesn't fail)
                if matches!(pipeline.state.status, ExecutionStatus::Failed) {
                    pipeline.state.status = ExecutionStatus::Running;
                }

                // Emit reroute event
                self.emit_event(ExecutionEvent::StepRerouted {
                    from_step: step_id.to_string(),
                    to_step: next_step.clone(),
                })
                .await;

                // Reset target step to Retrying if it was already completed/failed
                // Increment attempts to track re-execution due to routing
                let target_attempt = match pipeline.step(&next_step) {
                    Some(step) => match &step.state {
                        StepState::Completed { attempts, .. } | StepState::Failed { attempts, .. } => {
                            *attempts + 1
                        }
                        StepState::Pending | StepState::Retrying { .. } => 1,
                        _ => 1,
                    },
                    None => 1,
                };

                if let Some(step) = pipeline.step_mut(&next_step) {
                    if matches!(step.state, StepState::Completed { .. } | StepState::Failed { .. }) {
                        step.state = StepState::Retrying {
                            attempt: target_attempt,
                        };
                    }
                }

                // Enqueue the failure handler
                let mut scheduler = self.scheduler.lock().await;
                scheduler.enqueue(next_step);
            }
            ExecutionResult::Failed { error } => {
                self.mark_step_failed(pipeline, step_id, error, attempt).await;
            }
        }

        Ok(())
    }

    /// Handle continuation (retry or route)
    async fn handle_continuation(
        &self,
        pipeline: &mut Pipeline,
        step_id: &str,
        action: ContinueAction,
        _target: Option<String>,
    ) -> Result<(), String> {
        self.emit_event(ExecutionEvent::StepContinued {
            step_id: step_id.to_string(),
            action: action.clone(),
        })
        .await;

        match action {
            ContinueAction::Retry => {
                // Extract current attempt count before changing state
                let current_attempt = match pipeline.step(step_id) {
                    Some(step) => match &step.state {
                        StepState::Running { attempt, .. } => *attempt,
                        StepState::Retrying { attempt } => *attempt,
                        _ => 0,
                    },
                    None => 0,
                };

                // Increment attempt count for retry (each retry is a new execution attempt)
                let new_attempt = current_attempt + 1;

                // Set step to retrying state with incremented attempt
                if let Some(step) = pipeline.step_mut(step_id) {
                    step.state = StepState::Retrying {
                        attempt: new_attempt,
                    };
                }
                // Re-enqueue for execution
                let mut scheduler = self.scheduler.lock().await;
                scheduler.enqueue(step_id.to_string());
            }
            ContinueAction::Route(target_id) => {
                self.emit_event(ExecutionEvent::StepRerouted {
                    from_step: step_id.to_string(),
                    to_step: target_id.clone(),
                })
                .await;

                // Mark current step as completed (it's not a failure, just routing)
                // Get the current attempt count from the Running state
                let attempt = match pipeline.step(step_id) {
                    Some(step) => match &step.state {
                        StepState::Running { attempt, .. } => *attempt,
                        _ => 1,
                    },
                    None => 1,
                };

                if let Some(step) = pipeline.step_mut(step_id) {
                    let now = chrono::Utc::now();
                    step.state = StepState::Completed {
                        output: String::new(),
                        attempts: attempt,
                        started_at: now,
                        completed_at: now,
                    };
                }

                // Reset target step to Retrying state so it will execute again
                // If target was already completed, use its attempt count (don't increment)
                let target_attempt = match pipeline.step(&target_id) {
                    Some(step) => match &step.state {
                        StepState::Completed { attempts, .. } => *attempts,
                        StepState::Failed { attempts, .. } => *attempts,
                        StepState::Pending | StepState::Retrying { .. } => 1,
                        _ => 1,
                    },
                    None => 1,
                };

                if let Some(step) = pipeline.step_mut(&target_id) {
                    step.state = StepState::Retrying {
                        attempt: target_attempt,
                    };
                }

                // Enqueue target step
                let mut scheduler = self.scheduler.lock().await;
                scheduler.enqueue(target_id);
            }
        }

        Ok(())
    }

    /// Mark a step as completed successfully
    async fn mark_step_success(&self, pipeline: &mut Pipeline, step_id: &str, output: String) {
        if let Some(step) = pipeline.step_mut(step_id) {
            let (started_at, attempt) = match &step.state {
                StepState::Running { started_at, attempt } => (*started_at, *attempt),
                _ => (chrono::Utc::now(), 1),
            };

            step.state = StepState::Completed {
                output: output.clone(),
                attempts: attempt,
                started_at,
                completed_at: chrono::Utc::now(),
            };

            self.emit_event(ExecutionEvent::StepOutput {
                step_id: step_id.to_string(),
                output,
            })
            .await;
        }
    }

    /// Mark a step as failed
    async fn mark_step_failed(&self, pipeline: &mut Pipeline, step_id: &str, error: String, attempt: usize) {
        if let Some(step) = pipeline.step_mut(step_id) {
            let started_at = match &step.state {
                StepState::Running { started_at, .. } => *started_at,
                _ => chrono::Utc::now(),
            };

            step.state = StepState::Failed {
                error: error.clone(),
                attempts: attempt,
                last_started_at: started_at,
                failed_at: chrono::Utc::now(),
            };
        }

        self.emit_event(ExecutionEvent::StepFailed {
            step_id: step_id.to_string(),
            error,
        })
        .await;

        // Check if pipeline should fail overall (configurable - for now fail on any step failure)
        pipeline.state.fail();
    }

    /// Update pipeline state counts
    fn update_state_counts(&self, pipeline: &mut Pipeline) {
        let mut completed = 0;
        let mut failed = 0;
        let mut running = 0;

        for step in pipeline.steps.values() {
            match &step.state {
                StepState::Completed { .. } => completed += 1,
                StepState::Failed { .. } => failed += 1,
                StepState::Running { .. } => running += 1,
                _ => {}
            }
        }

        pipeline.state.update_counts(&pipeline.steps.len(), &completed, &failed, &running);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::PipelineConfig;
    use crate::agent::{AgentResponse, AgentError};
    use std::sync::Arc;

    // Mock agent for testing
    struct MockAgent {
        responses: Vec<String>,
        index: Arc<Mutex<usize>>,
    }

    impl MockAgent {
        fn new(responses: Vec<String>) -> Self {
            Self {
                responses,
                index: Arc::new(Mutex::new(0)),
            }
        }
    }

    #[async_trait::async_trait]
    impl AgentExecutor for MockAgent {
        async fn execute(&self, _prompt: &str) -> Result<AgentResponse, AgentError> {
            let mut idx = self.index.lock().await;
            if *idx < self.responses.len() {
                let response = self.responses[*idx].clone();
                *idx += 1;
                Ok(AgentResponse::new(response))
            } else {
                Ok(AgentResponse::new("DONE".to_string()))
            }
        }
    }

    #[tokio::test]
    async fn test_execute_simple_pipeline() {
        let yaml = r#"
name: "Test Pipeline"
steps:
  - id: "step1"
    name: "First"
    prompt: "Do task 1"
    termination:
      success_pattern: "DONE"
      on_success: "step2"
  - id: "step2"
    name: "Second"
    prompt: "Do task 2"
    termination:
      success_pattern: "DONE"
"#;

        let config = PipelineConfig::from_yaml(yaml).unwrap();
        let mut pipeline = config.to_pipeline();

        let agent = MockAgent::new(vec!["DONE".to_string(), "DONE".to_string()]);
        let engine = ExecutionEngine::new(agent, SchedulingStrategy::Sequential);

        let result = engine.execute(&mut pipeline).await;
        assert!(result.is_ok());
        assert!(pipeline.is_complete());
    }
}
