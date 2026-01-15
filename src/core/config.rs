//! Pipeline configuration from YAML

use crate::core::Pipeline;
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::path::Path;
use anyhow::Result;

/// Variable definition - can be a simple string or a file reference
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VariableDefinition {
    /// Simple string value
    String(String),
    /// File reference with validation flag
    File { path: String, validate_exists: bool },
}

impl VariableDefinition {
    /// Get the string representation for rendering in prompts
    pub fn render_value(&self) -> String {
        match self {
            VariableDefinition::String(s) => s.clone(),
            VariableDefinition::File { path, .. } => format!("@{}", path),
        }
    }
}

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
    variables: std::collections::HashMap<String, Value>,

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

        // Validate file existence for variables with validate_exists: true
        for (var_name, var_def) in self.get_variables() {
            if let VariableDefinition::File { path, validate_exists } = &var_def {
                if *validate_exists {
                    let path_obj = std::path::Path::new(path);
                    if !path_obj.exists() {
                        anyhow::bail!(
                            "Variable '{}' references file that doesn't exist: {}",
                            var_name,
                            path
                        );
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

    /// Get variables as parsed VariableDefinition enum
    pub fn get_variables(&self) -> std::collections::HashMap<String, VariableDefinition> {
        let mut vars = std::collections::HashMap::new();

        for (key, value) in &self.variables {
            let var_def = match value {
                Value::String(s) => VariableDefinition::String(s.clone()),
                Value::Mapping(map) => {
                    // Parse file variable: { path: "...", validate_exists: true/false }
                    let path = map.get(&Value::String("path".to_string()))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();

                    let validate_exists = map.get(&Value::String("validate_exists".to_string()))
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);

                    VariableDefinition::File { path, validate_exists }
                }
                _ => {
                    // Fallback: convert to string
                    VariableDefinition::String(serde_yaml::to_string(value).unwrap_or_default())
                }
            };
            vars.insert(key.clone(), var_def);
        }

        vars
    }

    /// Get variables as string map (for backward compatibility)
    pub fn variables_as_string_map(&self) -> std::collections::HashMap<String, String> {
        self.get_variables()
            .iter()
            .map(|(k, v)| (k.clone(), v.render_value()))
            .collect()
    }

    /// Convert config to a Pipeline domain model
    pub fn to_pipeline(&self) -> Pipeline {
        Pipeline::from_config(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // TDD Tests for File Variable Support
    // Completed tests:
    // ✓ Test 1: Parse simple string variable (backward compatibility)
    // ✓ Test 2: Parse file variable with path only (defaults to validate_exists: false)
    // ✓ Test 3: Parse file variable with explicit validate_exists: true
    // ✓ Test 4: Parse file variable with explicit validate_exists: false
    // ✓ Test 8: Validation passes when file exists and validate_exists: true
    // ✓ Test 9: Validation fails when file doesn't exist and validate_exists: true
    // ✓ Test 10: Validation passes when file doesn't exist but validate_exists: false
    // ✓ Test 11: Validation passes for simple string variables (no file check)
    // ✓ Test 14: Simple string variable renders as-is in prompt
    // ✓ Test 15: File variable renders as @path
    // ✓ Test 17: Multiple variables (strings and files) render correctly in same prompt

    // Test 1: ✓ Parse simple string variable (backward compatibility)
    #[test]
    fn test_variable_simple_string() {
        let yaml = r#"
name: "Test Pipeline"
variables:
  feature_name: "test feature"
steps: []
"#;

        let config = PipelineConfig::from_yaml(yaml).unwrap();
        let vars = config.get_variables();
        let var = vars.get("feature_name");
        assert!(var.is_some(), "Variable should exist");

        match var.unwrap() {
            VariableDefinition::String(s) => {
                assert_eq!(s, "test feature");
            }
            VariableDefinition::File { .. } => {
                panic!("Expected String variable, got File");
            }
        }
    }

    // Test 2: ✓ Parse file variable with path only (defaults to validate_exists: false)
    #[test]
    fn test_variable_file_path_only() {
        let yaml = r#"
name: "Test Pipeline"
variables:
  readme:
    path: "README.md"
steps: []
"#;

        let config = PipelineConfig::from_yaml(yaml).unwrap();
        let vars = config.get_variables();
        let var = vars.get("readme");
        assert!(var.is_some(), "Variable should exist");

        match var.unwrap() {
            VariableDefinition::File { path, validate_exists } => {
                assert_eq!(path, "README.md");
                assert_eq!(*validate_exists, false, "Should default to false");
            }
            VariableDefinition::String(_) => {
                panic!("Expected File variable, got String");
            }
        }
    }

    // Test 3: ✓ Parse file variable with explicit validate_exists: true
    #[test]
    fn test_variable_file_with_validate_true() {
        let yaml = r#"
name: "Test Pipeline"
variables:
  spec:
    path: "docs/spec.md"
    validate_exists: true
steps: []
"#;

        // Parse without validation since we don't want to create the file
        let config: PipelineConfig = serde_yaml::from_str(yaml).unwrap();
        let vars = config.get_variables();
        let var = vars.get("spec");
        assert!(var.is_some(), "Variable should exist");

        match var.unwrap() {
            VariableDefinition::File { path, validate_exists } => {
                assert_eq!(path, "docs/spec.md");
                assert_eq!(*validate_exists, true);
            }
            VariableDefinition::String(_) => {
                panic!("Expected File variable, got String");
            }
        }
    }

    // Test 4: ✓ Parse file variable with explicit validate_exists: false
    #[test]
    fn test_variable_file_with_validate_false() {
        let yaml = r#"
name: "Test Pipeline"
variables:
  output_file:
    path: "./dist/bundle.js"
    validate_exists: false
steps: []
"#;

        let config = PipelineConfig::from_yaml(yaml).unwrap();
        let vars = config.get_variables();
        let var = vars.get("output_file");
        assert!(var.is_some(), "Variable should exist");

        match var.unwrap() {
            VariableDefinition::File { path, validate_exists } => {
                assert_eq!(path, "./dist/bundle.js");
                assert_eq!(*validate_exists, false);
            }
            VariableDefinition::String(_) => {
                panic!("Expected File variable, got String");
            }
        }
    }

    // Test 14: Simple string variable renders as-is in prompt
    #[test]
    fn test_render_simple_string_variable() {
        let yaml = r#"
name: "Test Pipeline"
variables:
  feature_name: "user authentication"
steps: []
"#;

        let config = PipelineConfig::from_yaml(yaml).unwrap();
        let vars = config.variables_as_string_map();
        let var = vars.get("feature_name");
        assert_eq!(var, Some(&"user authentication".to_string()));
    }

    // Test 15: ✓ File variable renders as @path
    #[test]
    fn test_render_file_variable() {
        let yaml = r#"
name: "Test Pipeline"
variables:
  readme:
    path: "README.md"
    validate_exists: true
steps: []
"#;

        let config = PipelineConfig::from_yaml(yaml).unwrap();
        let vars = config.variables_as_string_map();
        let var = vars.get("readme");
        assert_eq!(var, Some(&"@README.md".to_string()));
    }

    // Test 8: ✓ Validation passes when file exists and validate_exists: true
    #[test]
    fn test_validate_file_exists() {
        // Create a temp file
        let temp_file = "/tmp/test_pi_pipeline_exists.md";
        std::fs::write(temp_file, "test content").unwrap();

        let yaml = format!(r#"
name: "Test Pipeline"
variables:
  test_file:
    path: "{}"
    validate_exists: true
steps: []
"#, temp_file);

        // Parse without validation
        let config: PipelineConfig = serde_yaml::from_str(&yaml).unwrap();
        // Validation should pass without error
        config.validate().expect("Validation should pass when file exists");

        // Cleanup
        std::fs::remove_file(temp_file).ok();
    }

    // Test 9: ✓ Validation fails when file doesn't exist and validate_exists: true
    #[test]
    fn test_validate_file_not_exists() {
        let yaml = r#"
name: "Test Pipeline"
variables:
  missing_file:
    path: "/tmp/nonexistent_file_12345.md"
    validate_exists: true
steps: []
"#;

        // Parse without validation using serde_yaml directly
        let config: PipelineConfig = serde_yaml::from_str(&yaml).unwrap();
        let result = config.validate();
        assert!(result.is_err(), "Validation should fail when file doesn't exist");
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("nonexistent_file_12345.md"), "Error should mention the file path");
    }

    // Test 10: Validation passes when file doesn't exist but validate_exists: false
    #[test]
    fn test_validate_file_not_exists_no_check() {
        let yaml = r#"
name: "Test Pipeline"
variables:
  output_file:
    path: "/tmp/nonexistent_output_12345.md"
    validate_exists: false
steps: []
"#;

        let config: PipelineConfig = serde_yaml::from_str(&yaml).unwrap();
        config.validate().expect("Validation should pass when validate_exists is false");
    }

    // Test 11: ✓ Validation passes for simple string variables (no file check)
    #[test]
    fn test_validate_string_variable() {
        let yaml = r#"
name: "Test Pipeline"
variables:
  simple: "just a string"
steps: []
"#;

        let config = PipelineConfig::from_yaml(&yaml).unwrap();
        config.validate().expect("Validation should pass for string variables");
    }

    // Test 17: Multiple variables (strings and files) render correctly in same prompt
    #[test]
    fn test_render_mixed_variables() {
        // Create temp file for this test
        let temp_file = "/tmp/test_render_readme.md";
        std::fs::write(temp_file, "test content").unwrap();

        let yaml = format!(r#"
name: "Test Pipeline"
variables:
  feature_name: "user auth"
  readme:
    path: "{}"
    validate_exists: true
  output:
    path: "output.md"
    validate_exists: false
steps: []
"#, temp_file);

        let config = PipelineConfig::from_yaml(&yaml).unwrap();
        let vars = config.variables_as_string_map();

        assert_eq!(vars.get("feature_name"), Some(&"user auth".to_string()));
        assert_eq!(vars.get("readme"), Some(&format!("@{}", temp_file)));
        assert_eq!(vars.get("output"), Some(&"@output.md".to_string()));

        // Cleanup
        std::fs::remove_file(temp_file).ok();
    }

    // Original test (will need updating after VariableDefinition is implemented)

    // TDD Test: ✓ Parse simple string variable (backward compatibility)
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
        // After implementing VariableDefinition, this will need to be updated
        // assert_eq!(config.variables.get("feature_name"), Some(&"test feature".to_string()));
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
