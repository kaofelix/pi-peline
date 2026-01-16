//! Smoke test - ensures basic pipeline functionality works end-to-end
//!
//! This test catches regressions that would break core functionality.
//! Run with: cargo test smoke_test

use pipeline::agent::{PiAgentClient, AgentClientConfig};
use pipeline::core::config::PipelineConfig;
use pipeline::execution::{ExecutionEngine, SchedulingStrategy};
use std::time::Duration;

/// Simple smoke test - runs a minimal pipeline and verifies it works
#[tokio::test]
#[ignore]  // Requires pi CLI
async fn smoke_test_basic_pipeline() {
    let yaml = r#"
name: "Smoke Test Pipeline"

variables:
  test_var: "test value"

steps:
  - id: "hello"
    name: "Say Hello"
    prompt: |
      Please respond with exactly: hello {{ test_var }}

      End your response with exactly: ✅ DONE
    termination:
      success_pattern: "✅ DONE"
"#;

    // Load and run pipeline
    let config = PipelineConfig::from_yaml(yaml).expect("Should parse YAML");
    let mut pipeline = config.to_pipeline();

    // Use Pi subprocess client (this will test session creation)
    let pi_client = PiAgentClient::new(AgentClientConfig::default());

    // Run with reasonable timeout
    let engine = ExecutionEngine::new(pi_client, SchedulingStrategy::Sequential, false);
    let start = std::time::Instant::now();

    let result = tokio::time::timeout(
        Duration::from_secs(60),
        engine.execute(&mut pipeline)
    ).await;

    let elapsed = start.elapsed();

    // Verify pipeline completed successfully
    match result {
        Ok(Ok(())) => {
            assert!(pipeline.is_complete(), "Pipeline should be complete");
            assert!(matches!(pipeline.state.status, pipeline::core::ExecutionStatus::Completed),
                    "Pipeline status should be Completed, got {:?}", pipeline.state.status);
        }
        Ok(Err(e)) => panic!("Pipeline execution failed: {:?}", e),
        Err(_) => panic!("Pipeline timed out after {}s", elapsed.as_secs()),
    }

    // Verify step output
    let step = pipeline.step("hello").expect("Step 'hello' should exist");
    match &step.state {
        pipeline::core::StepState::Completed { output, .. } => {
            assert!(!output.is_empty(), "Step output should not be empty");
            assert!(output.contains("hello"), "Output should contain 'hello'");
        }
        other => panic!("Step should be Completed, got {:?}", other),
    }

    // Verify pipeline completed in reasonable time
    assert!(elapsed < Duration::from_secs(30),
            "Pipeline should complete quickly, took {:?}", elapsed);

    println!("✅ Smoke test passed in {:?}", elapsed);
}

/// Smoke test for continuation/retry pattern
#[tokio::test]
#[ignore]  // Requires pi CLI
async fn smoke_test_continuation() {
    let yaml = r#"
name: "Smoke Test Continuation"

steps:
  - id: "task"
    name: "Task with Continuation"
    prompt: |
      Step 1: Say "working"
      End with: 🔄 CONTINUE

      Step 2: Say "done"
      End with: ✅ DONE
    termination:
      success_pattern: "✅ DONE"
    continuation:
      pattern: "🔄 CONTINUE"
      action: "retry"
"#;

    let config = PipelineConfig::from_yaml(yaml).expect("Should parse YAML");
    let mut pipeline = config.to_pipeline();

    let pi_client = PiAgentClient::new(AgentClientConfig::default());
    let engine = ExecutionEngine::new(pi_client, SchedulingStrategy::Sequential, false);

    let result = tokio::time::timeout(
        Duration::from_secs(60),
        engine.execute(&mut pipeline)
    ).await;

    assert!(result.is_ok() && result.unwrap().is_ok(),
            "Pipeline with continuation should complete");

    let step = pipeline.step("task").expect("Step should exist");
    match &step.state {
        pipeline::core::StepState::Completed { output, .. } => {
            assert!(output.contains("done"), "Output should contain final result");
        }
        other => panic!("Step should be Completed, got {:?}", other),
    }

    println!("✅ Continuation smoke test passed");
}

/// Smoke test for multi-step pipeline
#[tokio::test]
#[ignore]  // Requires pi CLI
async fn smoke_test_multi_step() {
    let yaml = r#"
name: "Smoke Test Multi-Step"

steps:
  - id: "step1"
    name: "Step 1"
    prompt: "Say: first. End with: ✅ STEP1_DONE"
    termination:
      success_pattern: "✅ STEP1_DONE"
      on_success: "step2"

  - id: "step2"
    name: "Step 2"
    depends_on: ["step1"]
    prompt: "Say: second. End with: ✅ STEP2_DONE"
    termination:
      success_pattern: "✅ STEP2_DONE"
"#;

    let config = PipelineConfig::from_yaml(yaml).expect("Should parse YAML");
    let mut pipeline = config.to_pipeline();

    let pi_client = PiAgentClient::new(AgentClientConfig::default());
    let engine = ExecutionEngine::new(pi_client, SchedulingStrategy::Sequential, false);

    let result = tokio::time::timeout(
        Duration::from_secs(90),
        engine.execute(&mut pipeline)
    ).await;

    assert!(result.is_ok() && result.unwrap().is_ok(),
            "Multi-step pipeline should complete");

    assert!(pipeline.is_complete(), "Pipeline should be complete");

    // Verify both steps completed
    for step_id in ["step1", "step2"] {
        let step = pipeline.step(step_id).expect(&format!("Step {} should exist", step_id));
        match &step.state {
            pipeline::core::StepState::Completed { output, .. } => {
                assert!(!output.is_empty(), "Step {} output should not be empty", step_id);
            }
            other => panic!("Step {} should be Completed, got {:?}", step_id, other),
        }
    }

    println!("✅ Multi-step smoke test passed");
}

/// Smoke test to verify JSON streaming doesn't produce parsing errors
#[tokio::test]
#[ignore]  // Requires pi CLI
async fn smoke_test_json_streaming() {
    // Use a longer prompt to trigger tool calls (file reads, etc)
    let yaml = r#"
name: "Smoke Test JSON Streaming"

variables:
  readme: "./README.md"

steps:
  - id: "analyze"
    name: "Analyze File"
    prompt: |
      Read {{ readme }} and write a brief 2-sentence summary.

      End your response with exactly: ✅ ANALYSIS_DONE
    termination:
      success_pattern: "✅ ANALYSIS_DONE"
"#;

    let config = PipelineConfig::from_yaml(yaml).expect("Should parse YAML");
    let mut pipeline = config.to_pipeline();

    let pi_client = PiAgentClient::new(AgentClientConfig::default());
    let engine = ExecutionEngine::new(pi_client, SchedulingStrategy::Sequential, false);

    let result = tokio::time::timeout(
        Duration::from_secs(60),
        engine.execute(&mut pipeline)
    ).await;

    assert!(result.is_ok() && result.unwrap().is_ok(),
            "JSON streaming pipeline should complete without parse errors");

    let step = pipeline.step("analyze").expect("Step should exist");
    match &step.state {
        pipeline::core::StepState::Completed { output, .. } => {
            assert!(!output.is_empty(), "Output should not be empty");
        }
        other => panic!("Step should be Completed, got {:?}", other),
    }

    println!("✅ JSON streaming smoke test passed");
}
