//! Termination condition model

use crate::core::step::ConditionPattern;

/// Termination condition for a step (not serializable due to ConditionPattern::Regex)
#[derive(Debug, Clone)]
pub struct TerminationCondition {
    /// Pattern that signals successful completion
    pub success_pattern: ConditionPattern,

    /// Which step to execute on success (None = end pipeline)
    pub on_success: Option<String>,

    /// Which step to execute on failure/rejection
    pub on_failure: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use regex::Regex;

    #[test]
    fn test_termination_condition() {
        let condition = TerminationCondition {
            success_pattern: ConditionPattern::Simple("DONE".to_string()),
            on_success: Some("next_step".to_string()),
            on_failure: Some("retry_step".to_string()),
        };

        assert!(condition.success_pattern.matches("Task DONE"));
        assert!(!condition.success_pattern.matches("Not done"));

        assert_eq!(condition.on_success, Some("next_step".to_string()));
        assert_eq!(condition.on_failure, Some("retry_step".to_string()));
    }

    #[test]
    fn test_termination_condition_with_regex() {
        let condition = TerminationCondition {
            success_pattern: ConditionPattern::Regex(Regex::new(r"✅\s*\w+").unwrap()),
            on_success: None,
            on_failure: None,
        };

        assert!(condition.success_pattern.matches("✅ COMPLETE"));
        assert!(condition.success_pattern.matches("✅   DONE"));
        assert!(!condition.success_pattern.matches("❌ FAILED"));
    }
}
