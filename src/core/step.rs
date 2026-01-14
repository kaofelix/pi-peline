//! Step domain model

use crate::core::{
    config::ContinuationAction,
    condition::TerminationCondition,
    state::StepState,
};
use regex::Regex;
use std::collections::HashMap;

/// A single step in a pipeline
#[derive(Debug, Clone)]
pub struct Step {
    /// Unique step identifier
    pub id: String,

    /// The base prompt template for this step
    pub prompt_template: String,

    /// List of step IDs this step depends on
    pub dependencies: Vec<String>,

    /// Termination condition (when step is considered complete)
    pub termination: Option<TerminationCondition>,

    /// Continuation condition (when step needs more work)
    pub continuation: Option<ContinuationCondition>,

    /// Maximum number of retries
    pub max_retries: usize,

    /// Timeout in seconds
    pub timeout_secs: u64,

    /// Runtime state (not serialized)
    pub state: StepState,
}

/// Continuation condition domain model
#[derive(Debug, Clone)]
pub struct ContinuationCondition {
    /// Compiled regex or simple string pattern
    pub pattern: ConditionPattern,

    /// Action to take when pattern matches
    pub action: crate::core::config::ContinuationAction,

    /// Target step ID when action is Route
    pub target: Option<String>,
}

/// Pattern for matching agent output (not serializable due to Regex)
#[derive(Debug, Clone)]
pub enum ConditionPattern {
    /// Simple string contains match
    Simple(String),
    /// Regular expression match
    Regex(Regex),
}

impl ConditionPattern {
    /// Check if the pattern matches the given text
    pub fn matches(&self, text: &str) -> bool {
        match self {
            ConditionPattern::Simple(pattern) => text.contains(pattern),
            ConditionPattern::Regex(regex) => regex.is_match(text),
        }
    }
}

impl Step {
    /// Create a step from a step config
    pub fn from_config(
        config: &crate::core::config::StepConfig,
        defaults: &StepDefaults,
    ) -> Self {
        let termination = config.termination.as_ref().map(|t| {
            let pattern = if t.use_regex {
                match Regex::new(&t.success_pattern) {
                    Ok(regex) => ConditionPattern::Regex(regex),
                    Err(_) => ConditionPattern::Simple(t.success_pattern.clone()),
                }
            } else {
                ConditionPattern::Simple(t.success_pattern.clone())
            };

            TerminationCondition {
                success_pattern: pattern,
                on_success: t.on_success.clone(),
                on_failure: t.on_failure.clone(),
            }
        });

        let continuation = config.continuation.as_ref().map(|c| {
            let pattern = if c.use_regex {
                match Regex::new(&c.pattern) {
                    Ok(regex) => ConditionPattern::Regex(regex),
                    Err(_) => ConditionPattern::Simple(c.pattern.clone()),
                }
            } else {
                ConditionPattern::Simple(c.pattern.clone())
            };

            ContinuationCondition {
                pattern,
                action: c.action,
                target: c.target.clone(),
            }
        });

        Step {
            id: config.id.clone(),
            prompt_template: config.prompt.clone(),
            dependencies: config.depends_on.clone(),
            termination,
            continuation,
            max_retries: config.max_retries.unwrap_or(defaults.max_retries),
            timeout_secs: config.timeout_secs.unwrap_or(defaults.timeout_secs),
            state: StepState::Pending,
        }
    }

    /// Check if all dependencies are satisfied (considering both completed and failed steps)
    pub fn dependencies_met(&self, completed_or_failed_steps: &HashSet<String>) -> bool {
        self.dependencies.iter().all(|dep| completed_or_failed_steps.contains(dep))
    }

    /// Render the prompt with variable substitution
    pub fn render_prompt(&self, variables: &HashMap<String, String>) -> String {
        let mut prompt = self.prompt_template.clone();

        // Replace variables in the form {{ variable_name }}
        for (key, value) in variables {
            let placeholder = format!("{{{{ {} }}}}", key);
            prompt = prompt.replace(&placeholder, value);
        }

        prompt
    }

    /// Build the effective prompt with termination/continuation instructions
    pub fn build_effective_prompt(&self, variables: &HashMap<String, String>) -> String {
        let mut instructions = String::new();
        let mut has_instructions = false;

        // Add termination instruction
        if let Some(termination) = &self.termination {
            instructions.push_str(&format!(
                "\n\n--- IMPORTANT: When you complete this task successfully, print exactly: {}\n",
                termination.success_pattern.display()
            ));
            has_instructions = true;
        }

        // Add continuation instruction
        if let Some(continuation) = &self.continuation {
            instructions.push_str(&format!(
                "If you need more work on this task, print exactly: {}\n",
                continuation.pattern.display()
            ));
            has_instructions = true;
        }

        // Add default behavior
        if !has_instructions {
            instructions.push_str(
                "\n\n--- IMPORTANT: When you complete this task, print: ✓ DONE\n",
            );
        }

        format!("{}{}", self.render_prompt(variables), instructions)
    }

    /// Check if agent output indicates successful completion
    pub fn is_success(&self, output: &str) -> bool {
        if let Some(termination) = &self.termination {
            termination.success_pattern.matches(output)
        } else {
            // Default: check for "✓ DONE" or just "DONE"
            output.contains("✓ DONE") || output.contains("DONE")
        }
    }

    /// Check if agent output indicates continuation needed
    pub fn needs_continuation(&self, output: &str) -> bool {
        if let Some(continuation) = &self.continuation {
            continuation.pattern.matches(output)
        } else {
            false
        }
    }

    /// Get the next step ID after successful completion
    pub fn next_step_on_success(&self) -> Option<&String> {
        self.termination.as_ref().and_then(|t| t.on_success.as_ref())
    }

    /// Get the next step ID after failure/rejection
    pub fn next_step_on_failure(&self) -> Option<&String> {
        self.termination.as_ref().and_then(|t| t.on_failure.as_ref())
    }

    /// Get continuation action and target
    pub fn get_continuation_action(&self) -> Option<(ContinuationAction, Option<&String>)> {
        self.continuation.as_ref().map(|c| (c.action, c.target.as_ref()))
    }
}

impl ConditionPattern {
    fn display(&self) -> String {
        match self {
            ConditionPattern::Simple(s) => s.clone(),
            ConditionPattern::Regex(r) => format!("[regex: {}]", r.as_str()),
        }
    }
}
#[derive(Debug, Clone)]
pub struct StepDefaults {
    pub max_retries: usize,
    pub timeout_secs: u64,
}

impl Default for StepDefaults {
    fn default() -> Self {
        Self {
            max_retries: 3,
            timeout_secs: 300, // 5 minutes
        }
    }
}

use std::collections::HashSet;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_prompt() {
        let step = Step {
            id: "test".to_string(),
            prompt_template: "Do {{ task }} with {{ item }}".to_string(),
            dependencies: vec![],
            termination: None,
            continuation: None,
            max_retries: 3,
            timeout_secs: 300,
            state: StepState::Pending,
        };

        let mut vars = HashMap::new();
        vars.insert("task".to_string(), "testing".to_string());
        vars.insert("item".to_string(), "code".to_string());

        let rendered = step.render_prompt(&vars);
        assert_eq!(rendered, "Do testing with code");
    }

    #[test]
    fn test_simple_pattern_matches() {
        let pattern = ConditionPattern::Simple("DONE".to_string());
        assert!(pattern.matches("Task is DONE"));
        assert!(!pattern.matches("Task is not finished"));
    }

    #[test]
    fn test_regex_pattern_matches() {
        let pattern = ConditionPattern::Regex(Regex::new(r"✅\s*\w+").unwrap());
        assert!(pattern.matches("✅ COMPLETE"));
        assert!(pattern.matches("✅   DONE"));
        assert!(!pattern.matches("❌ FAILED"));
    }
}
