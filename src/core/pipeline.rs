//! Pipeline domain model

use crate::core::{
    config::PipelineConfig,
    step::{Step, StepDefaults},
    state::{PipelineState, ExecutionStatus},
    context::PipelineContext,
};
use std::collections::{HashMap, HashSet};

/// A pipeline definition
#[derive(Debug, Clone)]
pub struct Pipeline {
    /// Pipeline name
    pub name: String,

    /// Global variables available to all steps
    pub variables: HashMap<String, String>,

    /// Pipeline steps
    pub steps: HashMap<String, Step>,

    /// Execution state
    pub state: PipelineState,

    /// Step execution order (topological sort) (not serialized)
    execution_order: Vec<String>,
}

impl Pipeline {
    /// Create a pipeline from configuration
    pub fn from_config(config: &PipelineConfig) -> Self {
        let defaults = StepDefaults {
            max_retries: config.max_retries.unwrap_or(3),
            timeout_secs: config.default_timeout_secs.unwrap_or(300),
        };

        let steps: HashMap<String, Step> = config
            .steps
            .iter()
            .map(|step_config| {
                let step = Step::from_config(step_config, &defaults);
                (step.id.clone(), step)
            })
            .collect();

        let execution_order = Self::topological_sort(&steps);

        Pipeline {
            name: config.name.clone(),
            variables: config.variables_as_string_map(),
            steps,
            state: PipelineState::new(),
            execution_order,
        }
    }

    /// Get a step by ID
    pub fn step(&self, id: &str) -> Option<&Step> {
        self.steps.get(id)
    }

    /// Get a mutable step by ID
    pub fn step_mut(&mut self, id: &str) -> Option<&mut Step> {
        self.steps.get_mut(id)
    }

    /// Get steps ready to execute (dependencies satisfied)
    pub fn ready_steps(&self) -> Vec<&Step> {
        let completed_or_failed: HashSet<String> = self
            .steps
            .values()
            .filter(|s| {
                matches!(s.state, crate::core::state::StepState::Completed { .. } | crate::core::state::StepState::Failed { .. })
            })
            .map(|s| s.id.clone())
            .collect();

        self.steps
            .values()
            .filter(|s| {
                matches!(s.state, crate::core::state::StepState::Pending | crate::core::state::StepState::Retrying { .. })
                    && s.dependencies_met(&completed_or_failed)
            })
            .collect()
    }

    /// Get all currently running steps
    pub fn running_steps(&self) -> Vec<&Step> {
        self.steps
            .values()
            .filter(|s| matches!(s.state, crate::core::state::StepState::Running { .. }))
            .collect()
    }

    /// Check if pipeline is complete
    pub fn is_complete(&self) -> bool {
        self.steps.values().all(|s| {
            matches!(
                s.state,
                crate::core::state::StepState::Completed { .. }
                    | crate::core::state::StepState::Skipped { .. }
                    | crate::core::state::StepState::Failed { .. }
            )
        })
    }

    /// Check if pipeline has failed
    pub fn has_failed(&self) -> bool {
        self.state.status == ExecutionStatus::Failed
    }

    /// Get execution order (topological sort)
    pub fn execution_order(&self) -> &[String] {
        &self.execution_order
    }

    /// Calculate topological sort of steps based on dependencies
    fn topological_sort(steps: &HashMap<String, Step>) -> Vec<String> {
        let mut result = Vec::new();
        let mut visited = HashSet::new();
        let mut temp_visited = HashSet::new();

        // Sort for deterministic order
        let mut step_ids: Vec<_> = steps.keys().cloned().collect();
        step_ids.sort();

        for step_id in step_ids {
            if !visited.contains(&step_id) {
                Self::visit(&step_id, steps, &mut visited, &mut temp_visited, &mut result);
            }
        }

        result
    }

    fn visit(
        step_id: &str,
        steps: &HashMap<String, Step>,
        visited: &mut HashSet<String>,
        temp_visited: &mut HashSet<String>,
        result: &mut Vec<String>,
    ) {
        if visited.contains(step_id) {
            return;
        }

        temp_visited.insert(step_id.to_string());

        if let Some(step) = steps.get(step_id) {
            for dep in &step.dependencies {
                Self::visit(dep, steps, visited, temp_visited, result);
            }
        }

        temp_visited.remove(step_id);
        visited.insert(step_id.to_string());
        result.push(step_id.to_string());
    }

    /// Create execution context for a step
    pub fn create_context_for_step(&self, step_id: &str) -> PipelineContext {
        let mut context = PipelineContext::new();

        // Add global variables
        context.variables.extend(self.variables.clone());

        // Add outputs from previous steps
        for (id, step) in &self.steps {
            if let crate::core::state::StepState::Completed { output, .. } = &step.state {
                context.set_step_output(id, output.clone());
            }
        }

        context.current_step_id = Some(step_id.to_string());

        context
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_topological_sort() {
        let yaml = r#"
name: "Test Pipeline"
steps:
  - id: "step1"
    name: "First"
    prompt: "Test"
  - id: "step2"
    name: "Second"
    prompt: "Test"
    depends_on: ["step1"]
  - id: "step3"
    name: "Third"
    prompt: "Test"
    depends_on: ["step1", "step2"]
"#;

        let config = PipelineConfig::from_yaml(yaml).unwrap();
        let pipeline = config.to_pipeline();

        let order = pipeline.execution_order();
        assert_eq!(order[0], "step1");
        assert!(order.iter().position(|x| x == "step1").unwrap() < order.iter().position(|x| x == "step2").unwrap());
        assert!(order.iter().position(|x| x == "step2").unwrap() < order.iter().position(|x| x == "step3").unwrap());
    }

    #[test]
    fn test_ready_steps() {
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
"#;

        let config = PipelineConfig::from_yaml(yaml).unwrap();
        let mut pipeline = config.to_pipeline();

        // Initially only step1 is ready
        let ready = pipeline.ready_steps();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].id, "step1");

        // Mark step1 as completed
        use crate::core::state::StepState;
        pipeline.step_mut("step1").unwrap().state = StepState::Completed {
            output: "DONE".to_string(),
            attempts: 1,
            started_at: chrono::Utc::now(),
            completed_at: chrono::Utc::now(),
        };

        // Now step2 is ready
        let ready = pipeline.ready_steps();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].id, "step2");
    }
}
