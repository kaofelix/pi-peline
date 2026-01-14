//! Test: Success Chain - Linear pipeline execution

use crate::helpers::*;
use pipeline::core::config::PipelineConfig;

/// Test that steps execute in order with success routing
#[tokio::test]
async fn test_success_chain() {
    let yaml = r#"
name: "Test: Success Chain"
description: "Verify steps execute in order with success routing"

steps:
  - id: "plan"
    name: "Create Plan"
    prompt: "Create implementation plan"
    termination:
      success_pattern: "✅ PLAN COMPLETE"
      on_success: "implement"

  - id: "implement"
    name: "Implement"
    depends_on: ["plan"]
    prompt: "Implement feature"
    termination:
      success_pattern: "✅ IMPLEMENTATION_DONE"
      on_success: "review"

  - id: "review"
    name: "Review"
    depends_on: ["implement"]
    prompt: "Review implementation"
    termination:
      success_pattern: "✅ APPROVED"
"#;

    let config = PipelineConfig::from_yaml(yaml).unwrap();
    let mut pipeline = config.to_pipeline();

    let responses = vec![
        "Creating the plan... ✅ PLAN COMPLETE".to_string(),
        "Implementing... ✅ IMPLEMENTATION_DONE".to_string(),
        "Reviewing... ✅ APPROVED".to_string(),
    ];

    let result = run_pipeline_with_mock(&mut pipeline, responses).await.unwrap();

    // Assert pipeline completed successfully
    assert_pipeline_completed(&result);

    // Assert steps executed in correct order
    assert_execution_order(&result, &["plan", "implement", "review"]);

    // Assert each step has correct output
    assert_step_executed(&result, "plan", "✅ PLAN COMPLETE");
    assert_step_executed(&result, "implement", "✅ IMPLEMENTATION_DONE");
    assert_step_executed(&result, "review", "✅ APPROVED");

    // Assert each step was executed exactly once
    assert_eq!(count_step_executions(&result, "plan"), 1);
    assert_eq!(count_step_executions(&result, "implement"), 1);
    assert_eq!(count_step_executions(&result, "review"), 1);

    // Assert no failures
    assert_eq!(result.failed_steps().len(), 0);
}

/// Test a longer success chain (5 steps)
#[tokio::test]
async fn test_success_chain_long() {
    let yaml = r#"
name: "Long Success Chain"

steps:
  - id: "step1"
    name: "Step 1"
    prompt: "First step"
    termination:
      success_pattern: "✅ STEP1_DONE"
      on_success: "step2"

  - id: "step2"
    name: "Step 2"
    prompt: "Second step"
    termination:
      success_pattern: "✅ STEP2_DONE"
      on_success: "step3"

  - id: "step3"
    name: "Step 3"
    prompt: "Third step"
    termination:
      success_pattern: "✅ STEP3_DONE"
      on_success: "step4"

  - id: "step4"
    name: "Step 4"
    prompt: "Fourth step"
    termination:
      success_pattern: "✅ STEP4_DONE"
      on_success: "step5"

  - id: "step5"
    name: "Step 5"
    prompt: "Fifth step"
    termination:
      success_pattern: "✅ STEP5_DONE"
"#;

    let config = PipelineConfig::from_yaml(yaml).unwrap();
    let mut pipeline = config.to_pipeline();

    let responses = vec![
        "✅ STEP1_DONE".to_string(),
        "✅ STEP2_DONE".to_string(),
        "✅ STEP3_DONE".to_string(),
        "✅ STEP4_DONE".to_string(),
        "✅ STEP5_DONE".to_string(),
    ];

    let result = run_pipeline_with_mock(&mut pipeline, responses).await.unwrap();

    assert_pipeline_completed(&result);
    assert_execution_order(&result, &["step1", "step2", "step3", "step4", "step5"]);

    for (step, pattern) in [
        ("step1", "✅ STEP1_DONE"),
        ("step2", "✅ STEP2_DONE"),
        ("step3", "✅ STEP3_DONE"),
        ("step4", "✅ STEP4_DONE"),
        ("step5", "✅ STEP5_DONE"),
    ] {
        assert_step_executed(&result, step, pattern);
        assert_eq!(count_step_executions(&result, step), 1);
    }
}

/// Test success chain without on_success (pipeline should end)
#[tokio::test]
async fn test_success_chain_end_without_routing() {
    let yaml = r#"
name: "End Without Routing"

steps:
  - id: "task"
    name: "Do Task"
    prompt: "Do something"
    termination:
      success_pattern: "✅ DONE"
"#;

    let config = PipelineConfig::from_yaml(yaml).unwrap();
    let mut pipeline = config.to_pipeline();

    let responses = vec!["✅ DONE".to_string()];

    let result = run_pipeline_with_mock(&mut pipeline, responses).await.unwrap();

    assert_pipeline_completed(&result);
    assert_step_executed(&result, "task", "✅ DONE");
    assert_eq!(result.completed_steps().len(), 1);
}

/// Test success chain with independent parallel steps
#[tokio::test]
async fn test_success_chain_with_parallel() {
    let yaml = r#"
name: "Parallel After Sequential"

steps:
  - id: "setup"
    name: "Setup"
    prompt: "Setup phase"
    termination:
      success_pattern: "✅ SETUP_DONE"

  - id: "task_a"
    name: "Task A"
    depends_on: ["setup"]
    prompt: "Task A"
    termination:
      success_pattern: "✅ TASK_A_DONE"

  - id: "task_b"
    name: "Task B"
    depends_on: ["setup"]
    prompt: "Task B"
    termination:
      success_pattern: "✅ TASK_B_DONE"
"#;

    let config = PipelineConfig::from_yaml(yaml).unwrap();
    let mut pipeline = config.to_pipeline();

    let responses = vec![
        "✅ SETUP_DONE".to_string(),
        "✅ TASK_A_DONE".to_string(),
        "✅ TASK_B_DONE".to_string(),
    ];

    let result = run_pipeline_with_mock(&mut pipeline, responses).await.unwrap();

    assert_pipeline_completed(&result);
    assert_eq!(result.completed_steps().len(), 3);

    // Verify setup was first
    assert_eq!(result.execution_order()[0], "setup");

    // Verify both tasks completed
    assert_step_executed(&result, "task_a", "✅ TASK_A_DONE");
    assert_step_executed(&result, "task_b", "✅ TASK_B_DONE");
}
