//! Execution scheduler - determines which steps to run next

use crate::core::{Pipeline, StepState};
use std::collections::{HashMap, HashSet, VecDeque};

/// Strategy for scheduling step execution
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedulingStrategy {
    /// Execute steps in dependency order, one at a time
    Sequential,

    /// Execute all ready steps in parallel
    Parallel,

    /// Limited parallelism (max N concurrent steps)
    LimitedParallel(usize),
}

impl Default for SchedulingStrategy {
    fn default() -> Self {
        SchedulingStrategy::Sequential
    }
}

/// Scheduler for determining which steps to run
pub struct ExecutionScheduler {
    strategy: SchedulingStrategy,
    explicit_queue: VecDeque<String>,
}

impl ExecutionScheduler {
    pub fn new(strategy: SchedulingStrategy) -> Self {
        Self {
            strategy,
            explicit_queue: VecDeque::new(),
        }
    }

    /// Add a step to the explicit execution queue
    pub fn enqueue(&mut self, step_id: String) {
        self.explicit_queue.push_back(step_id);
    }

    /// Get the next batch of steps to execute
    pub fn next_steps(&self, pipeline: &Pipeline) -> Vec<String> {
        // First check explicit queue
        if !self.explicit_queue.is_empty() {
            return self.collect_ready_from_queue(pipeline);
        }

        match self.strategy {
            SchedulingStrategy::Sequential => self.next_sequential(pipeline),
            SchedulingStrategy::Parallel => self.next_parallel(pipeline),
            SchedulingStrategy::LimitedParallel(max) => self.next_limited_parallel(pipeline, max),
        }
    }

    fn collect_ready_from_queue(&self, pipeline: &Pipeline) -> Vec<String> {
        let completed: HashSet<String> = pipeline
            .steps
            .values()
            .filter(|s| matches!(s.state, StepState::Completed { .. }))
            .map(|s| s.id.clone())
            .collect();

        let mut ready = Vec::new();

        for step_id in &self.explicit_queue {
            if let Some(step) = pipeline.step(step_id) {
                if matches!(step.state, StepState::Pending)
                    && step.dependencies_satisfied(&completed)
                {
                    ready.push(step_id.clone());
                    if self.strategy == SchedulingStrategy::Sequential {
                        break; // Only one at a time for sequential
                    }
                }
            }
        }

        ready
    }

    fn next_sequential(&self, pipeline: &Pipeline) -> Vec<String> {
        // Get the first ready step in execution order
        for step_id in pipeline.execution_order() {
            if let Some(step) = pipeline.step(step_id) {
                if matches!(step.state, StepState::Pending) {
                    let completed: HashSet<String> = pipeline
                        .steps
                        .values()
                        .filter(|s| matches!(s.state, StepState::Completed { .. }))
                        .map(|s| s.id.clone())
                        .collect();

                    if step.dependencies_satisfied(&completed) {
                        return vec![step_id.clone()];
                    }
                }
            }
        }

        vec![]
    }

    fn next_parallel(&self, pipeline: &Pipeline) -> Vec<String> {
        pipeline.ready_steps().iter().map(|s| s.id.clone()).collect()
    }

    fn next_limited_parallel(&self, pipeline: &Pipeline, max: usize) -> Vec<String> {
        let running_count = pipeline.running_steps().len();
        let remaining = max.saturating_sub(running_count);

        if remaining == 0 {
            return vec![];
        }

        pipeline
            .ready_steps()
            .into_iter()
            .take(remaining)
            .map(|s| s.id.clone())
            .collect()
    }

    /// Check if there are more steps to run
    pub fn has_more(&self, pipeline: &Pipeline) -> bool {
        !self.next_steps(pipeline).is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{config::{PipelineConfig, TerminationConfig}, state::StepState};
    use chrono::Utc;

    #[test]
    fn test_sequential_scheduler() {
        let yaml = r#"
name: "Test Pipeline"
steps:
  - id: "step1"
    name: "First"
    prompt: "Test"
    termination:
      success_pattern: "DONE"
  - id: "step2"
    name: "Second"
    prompt: "Test"
    depends_on: ["step1"]
    termination:
      success_pattern: "DONE"
  - id: "step3"
    name: "Third"
    prompt: "Test"
    depends_on: ["step2"]
    termination:
      success_pattern: "DONE"
"#;

        let config = PipelineConfig::from_yaml(yaml).unwrap();
        let mut pipeline = config.to_pipeline();
        let scheduler = ExecutionScheduler::new(SchedulingStrategy::Sequential);

        // Initially only step1 should be ready
        let next = scheduler.next_steps(&pipeline);
        assert_eq!(next, vec!["step1"]);
    }

    #[test]
    fn test_parallel_scheduler() {
        let yaml = r#"
name: "Test Pipeline"
steps:
  - id: "step1"
    name: "First"
    prompt: "Test"
    termination:
      success_pattern: "DONE"
  - id: "step2"
    name: "Second"
    prompt: "Test"
    termination:
      success_pattern: "DONE"
  - id: "step3"
    name: "Third"
    prompt: "Test"
    depends_on: ["step1", "step2"]
    termination:
      success_pattern: "DONE"
"#;

        let config = PipelineConfig::from_yaml(yaml).unwrap();
        let pipeline = config.to_pipeline();
        let scheduler = ExecutionScheduler::new(SchedulingStrategy::Parallel);

        // step1 and step2 should both be ready (no dependencies)
        let next = scheduler.next_steps(&pipeline);
        assert_eq!(next.len(), 2);
        assert!(next.contains(&"step1".to_string()));
        assert!(next.contains(&"step2".to_string()));
    }
}
