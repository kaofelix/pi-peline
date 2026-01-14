//! Test: Max Retries Exceeded - Verify retry limits are enforced

use crate::helpers::*;
use pipeline::core::config::PipelineConfig;

/// Test that pipeline fails when max_retries is exceeded
#[tokio::test]
async fn test_max_retries_exceeded() {
    let yaml = r#"
name: "Test: Max Retries Exceeded"
description: "Verify pipeline fails when max_retries is exceeded"

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

    // 4 responses all requesting retry (1 initial + 3 retries)
    let responses = vec![
        "Still working... 🔄 RETRY".to_string(),
        "More work... 🔄 RETRY".to_string(),
        "Even more... 🔄 RETRY".to_string(),
        "Still not done... 🔄 RETRY".to_string(),
    ];

    let result = run_pipeline_with_mock(&mut pipeline, responses).await.unwrap();

    // Assert pipeline failed
    assert_pipeline_failed(&result);

    // Assert task attempted 4 times (1 initial + 3 retries)
    assert_eq!(count_step_executions(&result, "task"), 4);

    // Assert failure reason mentions retries
    assert_step_failed(&result, "task", "retry");
}

/// Test max_retries of 0 (single attempt only)
#[tokio::test]
async fn test_max_retries_zero() {
    let yaml = r#"
name: "Test: Max Retries Zero"

steps:
  - id: "task"
    name: "Do Task"
    max_retries: 0
    prompt: "Do work"
    termination:
      success_pattern: "✅ DONE"
    continuation:
      pattern: "🔄 RETRY"
      action: "retry"
"#;

    let config = PipelineConfig::from_yaml(yaml).unwrap();
    let mut pipeline = config.to_pipeline();

    // Even though response requests retry, max_retries=0 prevents it
    let responses = vec!["Please retry 🔄 RETRY".to_string()];

    let result = run_pipeline_with_mock(&mut pipeline, responses).await.unwrap();

    // Should fail (no success pattern)
    assert_pipeline_failed(&result);

    // Only attempted once
    assert_eq!(count_step_executions(&result, "task"), 1);
}

/// Test max_retries of 1
#[tokio::test]
async fn test_max_retries_one() {
    let yaml = r#"
name: "Test: Max Retries One"

steps:
  - id: "task"
    name: "Do Task"
    max_retries: 1
    prompt: "Do work"
    termination:
      success_pattern: "✅ DONE"
    continuation:
      pattern: "🔄 RETRY"
      action: "retry"
"#;

    let config = PipelineConfig::from_yaml(yaml).unwrap();
    let mut pipeline = config.to_pipeline();

    // 2 requests, both retry
    let responses = vec![
        "First attempt 🔄 RETRY".to_string(),
        "Second attempt 🔄 RETRY".to_string(),
    ];

    let result = run_pipeline_with_mock(&mut pipeline, responses).await.unwrap();

    assert_pipeline_failed(&result);
    assert_eq!(count_step_executions(&result, "task"), 2);  // 1 initial + 1 retry
}

/// Test success on exactly max_retries attempt
#[tokio::test]
async fn test_success_on_last_retry() {
    let yaml = r#"
name: "Test: Success on Last Retry"

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

    // 3 retry requests, then success
    let responses = vec![
        "Attempt 1 🔄 RETRY".to_string(),
        "Attempt 2 🔄 RETRY".to_string(),
        "Attempt 3 🔄 RETRY".to_string(),
        "Attempt 4 ✅ DONE".to_string(),
    ];

    let result = run_pipeline_with_mock(&mut pipeline, responses).await.unwrap();

    assert_pipeline_completed(&result);
    assert_eq!(count_step_executions(&result, "task"), 4);  // 1 initial + 3 retries
    assert_step_executed(&result, "task", "✅ DONE");
}

/// Test large max_retries value
#[tokio::test]
async fn test_large_max_retries() {
    let yaml = r#"
name: "Test: Large Max Retries"

steps:
  - id: "task"
    name: "Do Task"
    max_retries: 10
    prompt: "Do work"
    termination:
      success_pattern: "✅ DONE"
    continuation:
      pattern: "🔄 RETRY"
      action: "retry"
"#;

    let config = PipelineConfig::from_yaml(yaml).unwrap();
    let mut pipeline = config.to_pipeline();

    // 5 retry requests, then success
    let responses = (0..5)
        .map(|i| format!("Attempt {} 🔄 RETRY", i + 1))
        .chain(std::iter::once("Success ✅ DONE".to_string()))
        .collect();

    let result = run_pipeline_with_mock(&mut pipeline, responses).await.unwrap();

    assert_pipeline_completed(&result);
    assert_eq!(count_step_executions(&result, "task"), 6);
    assert_step_executed(&result, "task", "✅ DONE");
}

/// Test mixed retry and success across multiple steps
#[tokio::test]
async fn test_retries_multiple_steps() {
    let yaml = r#"
name: "Test: Retries on Multiple Steps"

steps:
  - id: "step1"
    name: "Step 1"
    max_retries: 2
    prompt: "Step 1"
    termination:
      success_pattern: "✅ S1_DONE"
      on_success: "step2"
    continuation:
      pattern: "🔄 RETRY"
      action: "retry"

  - id: "step2"
    name: "Step 2"
    depends_on: ["step1"]
    max_retries: 1
    prompt: "Step 2"
    termination:
      success_pattern: "✅ S2_DONE"
      on_success: "step3"
    continuation:
      pattern: "🔄 RETRY"
      action: "retry"

  - id: "step3"
    name: "Step 3"
    depends_on: ["step2"]
    max_retries: 3
    prompt: "Step 3"
    termination:
      success_pattern: "✅ S3_DONE"
    continuation:
      pattern: "🔄 RETRY"
      action: "retry"
"#;

    let config = PipelineConfig::from_yaml(yaml).unwrap();
    let mut pipeline = config.to_pipeline();

    let responses = vec![
        // step1: retry once, then success
        "S1 attempt 1 🔄 RETRY".to_string(),
        "S1 attempt 2 ✅ S1_DONE".to_string(),
        // step2: succeed immediately
        "S2 attempt 1 ✅ S2_DONE".to_string(),
        // step3: retry twice, then success
        "S3 attempt 1 🔄 RETRY".to_string(),
        "S3 attempt 2 🔄 RETRY".to_string(),
        "S3 attempt 3 ✅ S3_DONE".to_string(),
    ];

    let result = run_pipeline_with_mock(&mut pipeline, responses).await.unwrap();

    assert_pipeline_completed(&result);
    assert_eq!(count_step_executions(&result, "step1"), 2);  // 1 initial + 1 retry
    assert_eq!(count_step_executions(&result, "step2"), 1);  // no retries
    assert_eq!(count_step_executions(&result, "step3"), 3);  // 1 initial + 2 retries

    assert_step_executed(&result, "step1", "✅ S1_DONE");
    assert_step_executed(&result, "step2", "✅ S2_DONE");
    assert_step_executed(&result, "step3", "✅ S3_DONE");
}

/// Test that max_retries is per-step, not global
#[tokio::test]
async fn test_max_retries_per_step() {
    let yaml = r#"
name: "Test: Max Retries Per Step"

steps:
  - id: "step1"
    name: "Step 1"
    max_retries: 2
    prompt: "Step 1"
    termination:
      success_pattern: "✅ S1_DONE"
      on_success: "step2"
    continuation:
      pattern: "🔄 RETRY"
      action: "retry"

  - id: "step2"
    name: "Step 2"
    depends_on: ["step1"]
    max_retries: 2
    prompt: "Step 2"
    termination:
      success_pattern: "✅ S2_DONE"
    continuation:
      pattern: "🔄 RETRY"
      action: "retry"
"#;

    let config = PipelineConfig::from_yaml(yaml).unwrap();
    let mut pipeline = config.to_pipeline();

    // Both steps should exhaust their retries independently
    let responses = vec![
        "S1 attempt 1 🔄 RETRY".to_string(),
        "S1 attempt 2 🔄 RETRY".to_string(),
        "S1 attempt 3 ✅ S1_DONE".to_string(),
        "S2 attempt 1 🔄 RETRY".to_string(),
        "S2 attempt 2 🔄 RETRY".to_string(),
        "S2 attempt 3 🔄 RETRY".to_string(),
    ];

    let result = run_pipeline_with_mock(&mut pipeline, responses).await.unwrap();

    // step1 succeeded, step2 failed
    assert_pipeline_failed(&result);
    assert_eq!(count_step_executions(&result, "step1"), 3);
    assert_eq!(count_step_executions(&result, "step2"), 3);

    assert_step_executed(&result, "step1", "✅ S1_DONE");
    assert_step_failed(&result, "step2", "retry");
}
