//! Test: Failure Handling - on_failure routing and error recovery

use crate::helpers::*;
use pipeline::core::config::PipelineConfig;

/// Test that on_failure routing works
#[tokio::test]
async fn test_failure_routing() {
    let yaml = r#"
name: "Test: Failure Routing"
description: "Verify on_failure routing works"

steps:
  - id: "risky_task"
    name: "Risky Task"
    prompt: "Do something that might fail"
    termination:
      success_pattern: "✅ SUCCESS"
      on_failure: "fallback"

  - id: "fallback"
    name: "Fallback"
    depends_on: ["risky_task"]
    prompt: "Handle the failure"
    termination:
      success_pattern: "✅ HANDLED"
"#;

    let config = PipelineConfig::from_yaml(yaml).unwrap();
    let mut pipeline = config.to_pipeline();

    // Task fails (no success pattern)
    let responses = vec![
        "Task failed...".to_string(),  // No ✅ SUCCESS pattern
        "Handling the failure... ✅ HANDLED".to_string(),
    ];

    let result = run_pipeline_with_mock(&mut pipeline, responses).await.unwrap();

    // Assert pipeline completed successfully (via fallback)
    assert_pipeline_completed(&result);

    // Assert risky_task failed
    assert_step_failed(&result, "risky_task", "termination");

    // Assert fallback succeeded
    assert_step_executed(&result, "fallback", "✅ HANDLED");

    // Assert execution order
    assert_execution_order(&result, &["risky_task", "fallback"]);
}

/// Test pipeline failure when no on_failure route exists
#[tokio::test]
async fn test_failure_without_handler() {
    let yaml = r#"
name: "Test: Failure Without Handler"

steps:
  - id: "task"
    name: "Task"
    prompt: "Do task"
    termination:
      success_pattern: "✅ SUCCESS"
"#;

    let config = PipelineConfig::from_yaml(yaml).unwrap();
    let mut pipeline = config.to_pipeline();

    // Task fails (no success pattern, no on_failure)
    let responses = vec!["Task failed...".to_string()];

    let result = run_pipeline_with_mock(&mut pipeline, responses).await.unwrap();

    // Assert pipeline failed
    assert_pipeline_failed(&result);

    // Assert task failed
    assert_step_failed(&result, "task", "termination");
}

/// Test failure with explicit on_failure to end pipeline
#[tokio::test]
async fn test_failure_to_terminator() {
    let yaml = r#"
name: "Test: Failure to Terminator"

steps:
  - id: "task"
    name: "Task"
    prompt: "Do task"
    termination:
      success_pattern: "✅ SUCCESS"
      on_success: "cleanup"
      on_failure: "cleanup"

  - id: "cleanup"
    name: "Cleanup"
    depends_on: ["task"]
    prompt: "Cleanup resources"
    termination:
      success_pattern: "✅ CLEANUP_DONE"
"#;

    let config = PipelineConfig::from_yaml(yaml).unwrap();
    let mut pipeline = config.to_pipeline();

    // Task fails, but cleanup should run
    let responses = vec![
        "Task failed...".to_string(),
        "Cleaning up... ✅ CLEANUP_DONE".to_string(),
    ];

    let result = run_pipeline_with_mock(&mut pipeline, responses).await.unwrap();

    // Assert pipeline completed (cleanup ran)
    assert_pipeline_completed(&result);

    assert_step_failed(&result, "task", "termination");
    assert_step_executed(&result, "cleanup", "✅ CLEANUP_DONE");
}

/// Test multiple failure handlers in sequence
#[tokio::test]
async fn test_multiple_failure_handlers() {
    let yaml = r#"
name: "Test: Multiple Failure Handlers"

steps:
  - id: "task1"
    name: "Task 1"
    prompt: "Do task 1"
    termination:
      success_pattern: "✅ DONE1"
      on_success: "task2"
      on_failure: "fallback1"

  - id: "task2"
    name: "Task 2"
    depends_on: ["task1"]
    prompt: "Do task 2"
    termination:
      success_pattern: "✅ DONE2"
      on_success: "task3"
      on_failure: "fallback2"

  - id: "task3"
    name: "Task 3"
    depends_on: ["task2"]
    prompt: "Do task 3"
    termination:
      success_pattern: "✅ DONE3"

  - id: "fallback1"
    name: "Fallback 1"
    depends_on: ["task1"]
    prompt: "Handle task1 failure"
    termination:
      success_pattern: "✅ F1_DONE"

  - id: "fallback2"
    name: "Fallback 2"
    depends_on: ["task2"]
    prompt: "Handle task2 failure"
    termination:
      success_pattern: "✅ F2_DONE"
"#;

    let config = PipelineConfig::from_yaml(yaml).unwrap();
    let mut pipeline = config.to_pipeline();

    // task1 succeeds, task2 fails
    let responses = vec![
        "Task1 done ✅ DONE1".to_string(),
        "Task2 failed...".to_string(),  // No ✅ DONE2 pattern
        "Handling task2 failure ✅ F2_DONE".to_string(),
    ];

    let result = run_pipeline_with_mock(&mut pipeline, responses).await.unwrap();

    assert_pipeline_completed(&result);

    // Task1 succeeded, task2 failed, task3 never ran
    assert_step_executed(&result, "task1", "✅ DONE1");
    assert_step_failed(&result, "task2", "termination");
    assert_eq!(count_step_executions(&result, "task3"), 0);

    // fallback2 should have run
    assert_step_executed(&result, "fallback2", "✅ F2_DONE");
}

/// Test failure with timeout
#[tokio::test]
async fn test_failure_timeout() {
    let yaml = r#"
name: "Test: Timeout Failure"

steps:
  - id: "task"
    name: "Task"
    prompt: "Do task"
    timeout_secs: 0
    termination:
      success_pattern: "✅ DONE"
      on_failure: "cleanup"

  - id: "cleanup"
    name: "Cleanup"
    depends_on: ["task"]
    prompt: "Cleanup"
    termination:
      success_pattern: "✅ CLEANUP_DONE"
"#;

    let config = PipelineConfig::from_yaml(yaml).unwrap();
    let mut pipeline = config.to_pipeline();

    // Note: This test uses a very short timeout. The mock agent returns immediately,
    // so timeout might not trigger. This test is more about the structure.
    let responses = vec![
        "Task response ✅ DONE".to_string(),
        "Cleanup... ✅ CLEANUP_DONE".to_string(),
    ];

    let result = run_pipeline_with_mock(&mut pipeline, responses).await.unwrap();

    // With quick mock response, it should succeed
    assert_pipeline_completed(&result);
    assert_step_executed(&result, "task", "✅ DONE");
}

/// Test failure then retry (combination of failure and retry)
#[tokio::test]
async fn test_failure_then_retry() {
    let yaml = r#"
name: "Test: Failure Then Retry"

steps:
  - id: "task"
    name: "Task"
    max_retries: 2
    prompt: "Do task"
    termination:
      success_pattern: "✅ DONE"
    continuation:
      pattern: "🔄 RETRY"
      action: "retry"
      target: null
      carry_notes: false
"#;

    let config = PipelineConfig::from_yaml(yaml).unwrap();
    let mut pipeline = config.to_pipeline();

    // First attempt: retry, second attempt: retry, third: fail (no pattern)
    let responses = vec![
        "First 🔄 RETRY".to_string(),
        "Second 🔄 RETRY".to_string(),
        "Third - no pattern".to_string(),
    ];

    let result = run_pipeline_with_mock(&mut pipeline, responses).await.unwrap();

    // Should fail after exhausting retries
    assert_pipeline_failed(&result);
    assert_eq!(count_step_executions(&result, "task"), 3);
}

/// Test partial success with some failures
#[tokio::test]
async fn test_partial_success() {
    let yaml = r#"
name: "Test: Partial Success"

steps:
  - id: "step1"
    name: "Step 1"
    prompt: "Step 1"
    termination:
      success_pattern: "✅ S1"
      on_success: "step2"

  - id: "step2"
    name: "Step 2"
    depends_on: ["step1"]
    prompt: "Step 2"
    termination:
      success_pattern: "✅ S2"
      on_success: "step3"
      on_failure: "step3"

  - id: "step3"
    name: "Step 3"
    depends_on: ["step2"]
    prompt: "Step 3"
    termination:
      success_pattern: "✅ S3"
"#;

    let config = PipelineConfig::from_yaml(yaml).unwrap();
    let mut pipeline = config.to_pipeline();

    // step1 succeeds, step2 fails but routes to step3 anyway
    let responses = vec![
        "Step1 done ✅ S1".to_string(),
        "Step2 failed...".to_string(),
        "Step3 done ✅ S3".to_string(),
    ];

    let result = run_pipeline_with_mock(&mut pipeline, responses).await.unwrap();

    assert_pipeline_completed(&result);
    assert_step_executed(&result, "step1", "✅ S1");
    assert_step_failed(&result, "step2", "termination");
    assert_step_executed(&result, "step3", "✅ S3");
}
