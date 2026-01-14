//! Pipeline configuration from YAML

use crate::core::Pipeline;
use serde::{Deserialize, Serialize};
use std::path::Path;
use anyhow::Result;

/// Top-level pipeline configuration loaded from YAML
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineConfig {
    /// Pipeline name
    pub name: String,

    /// Pipeline version (optional)
    #[serde(default)]
    pub version: Option<String>,

    /// Global variables available to all steps
    #[serde(default)]
    pub variables: std::collections::HashMap<String, String>,

    /// Pipeline steps
    pub steps: Vec<StepConfig>,

    /// Maximum number of retries per step (global default)
    #[serde(default)]
    pub max_retries: Option<usize>,

    /// Default timeout for steps (in seconds)
    #[serde(default)]
    pub default_timeout_secs: Option<u64>,
}

/// Step configuration as defined in YAML
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepConfig {
    /// Unique step identifier
    pub id: String,

    /// Human-readable step name
    pub name: String,

    /// Optional step description
    #[serde(default)]
    pub description: Option<String>,

    /// The prompt template for this step
    pub prompt: String,

    /// List of step IDs this step depends on
    #[serde(default)]
    pub depends_on: Vec<String>,

    /// Termination condition configuration
    #[serde(default)]
    pub termination: Option<TerminationConfig>,

    /// Continuation condition configuration
    #[serde(default)]
    pub continuation: Option<ContinuationConfig>,

    /// Maximum retries for this step (overrides global)
    #[serde(default)]
    pub max_retries: Option<usize>,

    /// Timeout for this step (overrides global)
    #[serde(default)]
    pub timeout_secs: Option<u64>,

    /// Whether this step can run in parallel with others
    #[serde(default)]
    pub allow_parallel: bool,
}

/// Termination condition configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminationConfig {
    /// Pattern that signals successful completion
    pub success_pattern: String,

    /// Which step to execute on success (null = end pipeline)
    #[serde(default)]
    pub on_success: Option<String>,

    /// Which step to execute on failure/rejection
    #[serde(default)]
    pub on_failure: Option<String>,

    /// Whether to use regex pattern matching
    #[serde(default)]
    pub use_regex: bool,
}

/// Continuation condition configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContinuationConfig {
    /// Pattern that signals "not done, continue"
    pub pattern: String,

    /// Action to take: "retry" (same step) or "route" (different step)
    #[serde(default = "default_continuation_action")]
    pub action: ContinuationAction,

    /// Target step ID when action is "route"
    #[serde(default)]
    pub target: Option<String>,

    /// Whether to pass notes/context when routing
    #[serde(default)]
    pub carry_notes: bool,

    /// Whether to use regex pattern matching
    #[serde(default)]
    pub use_regex: bool,
}

/// Action to take when continuation pattern is matched
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ContinuationAction {
    /// Retry the same step with another turn
    Retry,
    /// Route to a different step
    Route,
}

fn default_continuation_action() -> ContinuationAction {
    ContinuationAction::Retry
}

impl PipelineConfig {
    /// Load pipeline configuration from a YAML file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Self::from_yaml(&content)
    }

    /// Parse pipeline configuration from YAML string
    pub fn from_yaml(yaml: &str) -> Result<Self> {
        let config: PipelineConfig = serde_yaml::from_str(yaml)?;
        config.validate()?;
        Ok(config)
    }

    /// Validate the pipeline configuration
    pub fn validate(&self) -> Result<()> {
        // Check that all step IDs are unique
        let mut seen_ids = std::collections::HashSet::new();
        for step in &self.steps {
            if !seen_ids.insert(&step.id) {
                anyhow::bail!("Duplicate step ID: {}", step.id);
            }
        }

        // Check that all dependencies reference existing steps
        let step_ids: std::collections::HashSet<_> = self.steps.iter().map(|s| &s.id).collect();
        for step in &self.steps {
            for dep in &step.depends_on {
                if !step_ids.contains(dep) {
                    anyhow::bail!(
                        "Step '{}' depends on non-existent step '{}'",
                        step.id,
                        dep
                    );
                }
            }

            // Validate termination targets
            if let Some(termination) = &step.termination {
                if let Some(ref on_success) = termination.on_success {
                    if !step_ids.contains(on_success) {
                        anyhow::bail!(
                            "Step '{}' termination on_success references non-existent step '{}'",
                            step.id,
                            on_success
                        );
                    }
                }
                if let Some(ref on_failure) = termination.on_failure {
                    if !step_ids.contains(on_failure) {
                        anyhow::bail!(
                            "Step '{}' termination on_failure references non-existent step '{}'",
                            step.id,
                            on_failure
                        );
                    }
                }
            }

            // Validate continuation target
            if let Some(continuation) = &step.continuation {
                if continuation.action == ContinuationAction::Route {
                    if continuation.target.is_none() {
                        anyhow::bail!(
                            "Step '{}' has continuation action 'route' but no target specified",
                            step.id
                        );
                    }
                    if let Some(ref target) = continuation.target {
                        if !step_ids.contains(target) {
                            anyhow::bail!(
                                "Step '{}' continuation target references non-existent step '{}'",
                                step.id,
                                target
                            );
                        }
                    }
                }
            }
        }

        // Check for cycles in the dependency graph (only depends_on, not termination/continuation)
        self.check_cycles()?;

        Ok(())
    }

    /// Check for cycles in the step dependency graph
    ///
    /// Note: This only checks `depends_on` relationships for cycles.
    /// Cycles through termination/continuation targets are allowed
    /// as they are intentional (e.g., review → implementation loops).
    fn check_cycles(&self) -> Result<()> {
        let mut visited = std::collections::HashSet::new();
        let mut recursion_stack = std::collections::HashSet::new();

        for step in &self.steps {
            if !visited.contains(&step.id) {
                self.dfs_check(&step.id, &mut visited, &mut recursion_stack)?;
            }
        }

        Ok(())
    }

    fn dfs_check(
        &self,
        step_id: &str,
        visited: &mut std::collections::HashSet<String>,
        recursion_stack: &mut std::collections::HashSet<String>,
    ) -> Result<()> {
        visited.insert(step_id.to_string());
        recursion_stack.insert(step_id.to_string());

        if let Some(step) = self.steps.iter().find(|s| s.id == step_id) {
            // Check dependencies only (not termination/continuation targets)
            // as those can create intentional loops like review → implementation
            for dep in &step.depends_on {
                if recursion_stack.contains(dep) {
                    anyhow::bail!("Cycle detected in dependency graph involving step '{}'", dep);
                }
                if !visited.contains(dep) {
                    self.dfs_check(dep, visited, recursion_stack)?;
                }
            }
        }

        recursion_stack.remove(step_id);
        Ok(())
    }

    /// Convert config to a Pipeline domain model
    pub fn to_pipeline(&self) -> Pipeline {
        Pipeline::from_config(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_pipeline() {
        let yaml = r#"
name: "Test Pipeline"
version: "1.0"

variables:
  feature_name: "test feature"

steps:
  - id: "step1"
    name: "First Step"
    prompt: "Do something with {{ feature_name }}"
    termination:
      success_pattern: "DONE"
      on_success: "step2"

  - id: "step2"
    name: "Second Step"
    depends_on: ["step1"]
    prompt: "Follow up"
    termination:
      success_pattern: "DONE"
"#;

        let config = PipelineConfig::from_yaml(yaml).unwrap();
        assert_eq!(config.name, "Test Pipeline");
        assert_eq!(config.steps.len(), 2);
        assert_eq!(config.variables.get("feature_name"), Some(&"test feature".to_string()));
    }

    #[test]
    fn test_duplicate_step_id_fails() {
        let yaml = r#"
name: "Test Pipeline"
steps:
  - id: "step1"
    name: "First"
    prompt: "Test"
  - id: "step1"
    name: "Duplicate"
    prompt: "Test"
"#;

        assert!(PipelineConfig::from_yaml(yaml).is_err());
    }

    #[test]
    fn test_invalid_dependency_fails() {
        let yaml = r#"
name: "Test Pipeline"
steps:
  - id: "step1"
    name: "First"
    prompt: "Test"
    depends_on: ["nonexistent"]
"#;

        assert!(PipelineConfig::from_yaml(yaml).is_err());
    }
}
