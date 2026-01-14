//! Test: Retry Behavior - Continuation with retry action

use crate::helpers::*;
use pipeline::core::config::PipelineConfig;

/// Test that retry action works and respects max_retries
#[tokio::test]
async fn test_retry_behavior() {
    let yaml = r#"
name: "Test: Retry on Continuation"
description: "Verify retry action works and respects max_retries"

steps:
  - id: "task"
    name: "Do Task"
    max_retries: 3
    prompt: "Do some work"
    termination:
      success_pattern: "✅ DONE"
    continuation:
      pattern: "🔄 CONTINUE"
      action: "retry"
"#;

    let config = PipelineConfig::from_yaml(yaml).unwrap();
    let mut pipeline = config.to_pipeline();

    let responses = vec![
        "Working on it... 🔄 CONTINUE".to_string(),
        "Still working... 🔄 CONTINUE".to_string(),
        "Finished! ✅ DONE".to_string(),
    ];

    let result = run_pipeline_with_mock(&mut pipeline, responses).await.unwrap();

    // Assert pipeline completed successfully
    assert_pipeline_completed(&result);

    // Assert task was executed 3 times (2 retries + 1 success)
    assert_eq!(count_step_executions(&result, "task"), 3);

    // Assert final output contains success pattern
    assert_step_executed(&result, "task", "✅ DONE");
}

/// Test retry with max retries exactly met
#[tokio::test]
async fn test_retry_max_retries_exact() {
    let yaml = r#"
name: "Test: Max Retries Exact"

steps:
  - id: "task"
    name: "Do Task"
    max_retries: 3
    prompt: "Do work"
    termination:
      success_pattern: "✅ DONE"
    continuation:
      pattern: "🔄 RETRY"
      action: "retry"
"#;

    let config = PipelineConfig::from_yaml(yaml).unwrap();
    let mut pipeline = config.to_pipeline();

    // Exactly 3 responses, last one succeeds
    let responses = vec![
        "First attempt 🔄 RETRY".to_string(),
        "Second attempt 🔄 RETRY".to_string(),
        "Third attempt ✅ DONE".to_string(),
    ];

    let result = run_pipeline_with_mock(&mut pipeline, responses).await.unwrap();

    assert_pipeline_completed(&result);
    assert_eq!(count_step_executions(&result, "task"), 3);
    assert_step_executed(&result, "task", "✅ DONE");
}

/// Test retry with single attempt (no retries)
#[tokio::test]
async fn test_retry_single_attempt() {
    let yaml = r#"
name: "Test: Single Attempt Success"

steps:
  - id: "task"
    name: "Do Task"
    max_retries: 0
    prompt: "Do work"
    termination:
      success_pattern: "✅ DONE"
"#;

    let config = PipelineConfig::from_yaml(yaml).unwrap();
    let mut pipeline = config.to_pipeline();

    let responses = vec!["✅ DONE".to_string()];

    let result = run_pipeline_with_mock(&mut pipeline, responses).await.unwrap();

    assert_pipeline_completed(&result);
    assert_eq!(count_step_executions(&result, "task"), 1);
    assert_step_executed(&result, "task", "✅ DONE");
}

/// Test retry with mixed success/continuation pattern
#[tokio::test]
async fn test_retry_mixed_patterns() {
    let yaml = r#"
name: "Test: Mixed Patterns"

steps:
  - id: "task"
    name: "Do Task"
    max_retries: 5
    prompt: "Do work"
    termination:
      success_pattern: "✅ FINISHED"
    continuation:
      pattern: "🔄 CONTINUE"
      action: "retry"
"#;

    let config = PipelineConfig::from_yaml(yaml).unwrap();
    let mut pipeline = config.to_pipeline();

    let responses = vec![
        "Starting... 🔄 CONTINUE".to_string(),
        "Progress... 🔄 CONTINUE".to_string(),
        "Almost done... 🔄 CONTINUE".to_string(),
        "Still working... 🔄 CONTINUE".to_string(),
        "✅ FINISHED".to_string(),
    ];

    let result = run_pipeline_with_mock(&mut pipeline, responses).await.unwrap();

    assert_pipeline_completed(&result);
    assert_eq!(count_step_executions(&result, "task"), 5);
    assert_step_executed(&result, "task", "✅ FINISHED");
}

/// Test that continuation pattern takes precedence over success pattern
#[tokio::test]
async fn test_retry_precedence() {
    let yaml = r#"
name: "Test: Continuation Precedence"

steps:
  - id: "task"
    name: "Do Task"
    max_retries: 2
    prompt: "Do work"
    termination:
      success_pattern: "✅ DONE"
    continuation:
      pattern: "🔄 CONTINUE"
      action: "retry"
"#;

    let config = PipelineConfig::from_yaml(yaml).unwrap();
    let mut pipeline = config.to_pipeline();

    // Response contains both patterns - continuation should take precedence
    let responses = vec![
        "Has both patterns: ✅ DONE 🔄 CONTINUE".to_string(),
        "Now just ✅ DONE".to_string(),
    ];

    let result = run_pipeline_with_mock(&mut pipeline, responses).await.unwrap();

    assert_pipeline_completed(&result);
    assert_eq!(count_step_executions(&result, "task"), 2);
}
