//! Pipeline context - shared state and variables

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Execution context for a pipeline run
///
/// Contains shared variables, step outputs, and other runtime data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineContext {
    /// Global and user-defined variables
    pub variables: HashMap<String, String>,

    /// Outputs from completed steps (step_id -> output)
    pub step_outputs: HashMap<String, String>,

    /// The current step being executed (if any)
    pub current_step_id: Option<String>,

    /// Notes or feedback passed between steps (e.g., review → implementation)
    pub notes: Vec<ContextNote>,

    /// Metadata about the execution
    pub metadata: HashMap<String, String>,
}

/// A note or piece of feedback in the context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextNote {
    /// The note content
    pub content: String,

    /// Which step created this note
    pub from_step: String,

    /// When the note was created
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl PipelineContext {
    /// Create a new empty context
    pub fn new() -> Self {
        Self {
            variables: HashMap::new(),
            step_outputs: HashMap::new(),
            current_step_id: None,
            notes: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    /// Set a variable
    pub fn set_variable(&mut self, key: String, value: String) {
        self.variables.insert(key, value);
    }

    /// Get a variable
    pub fn get_variable(&self, key: &str) -> Option<&String> {
        self.variables.get(key)
    }

    /// Set the output of a step
    pub fn set_step_output(&mut self, step_id: &str, output: String) {
        self.step_outputs.insert(step_id.to_string(), output);
    }

    /// Get the output of a step
    pub fn get_step_output(&self, step_id: &str) -> Option<&String> {
        self.step_outputs.get(step_id)
    }

    /// Add a note to the context
    pub fn add_note(&mut self, content: String, from_step: String) {
        let note = ContextNote {
            content,
            from_step,
            timestamp: chrono::Utc::now(),
        };
        self.notes.push(note);
    }

    /// Get all notes as a formatted string
    pub fn format_notes(&self) -> String {
        if self.notes.is_empty() {
            String::new()
        } else {
            self.notes
                .iter()
                .enumerate()
                .map(|(i, note)| {
                    format!(
                        "{}. [from {}] {}\n",
                        i + 1,
                        note.from_step,
                        note.content
                    )
                })
                .collect()
        }
    }

    /// Get all variables available for prompt rendering
    pub fn get_rendering_variables(&self) -> HashMap<String, String> {
        let mut vars = self.variables.clone();

        // Add step outputs as variables
        for (step_id, output) in &self.step_outputs {
            vars.insert(format!("steps.{}.output", step_id), output.clone());
        }

        // Add current step
        if let Some(ref current_step) = self.current_step_id {
            vars.insert("current_step".to_string(), current_step.clone());
        }

        // Add notes
        if !self.notes.is_empty() {
            vars.insert("notes".to_string(), self.format_notes());
        }

        vars
    }
}

impl Default for PipelineContext {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_variables() {
        let mut ctx = PipelineContext::new();
        ctx.set_variable("foo".to_string(), "bar".to_string());

        assert_eq!(ctx.get_variable("foo"), Some(&"bar".to_string()));
        assert_eq!(ctx.get_variable("baz"), None);
    }

    #[test]
    fn test_step_outputs() {
        let mut ctx = PipelineContext::new();
        ctx.set_step_output("step1", "output of step1".to_string());

        assert_eq!(
            ctx.get_step_output("step1"),
            Some(&"output of step1".to_string())
        );

        let vars = ctx.get_rendering_variables();
        assert_eq!(
            vars.get("steps.step1.output"),
            Some(&"output of step1".to_string())
        );
    }

    #[test]
    fn test_notes() {
        let mut ctx = PipelineContext::new();
        ctx.add_note("Fix the bug".to_string(), "review".to_string());

        let formatted = ctx.format_notes();
        assert!(formatted.contains("Fix the bug"));
        assert!(formatted.contains("review"));

        let vars = ctx.get_rendering_variables();
        assert!(vars.get("notes").unwrap().contains("Fix the bug"));
    }
}
