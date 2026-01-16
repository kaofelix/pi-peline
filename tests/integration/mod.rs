//! Integration tests with real Pi CLI subprocess
//!
//! These tests require the `pi` CLI to be installed and accessible.
//! They are tagged with `#[ignore]` and should be run explicitly with:
//!
//!     cargo test --ignored
//!
//! or:
//!
//!     cargo test --test integration --ignored

use pipeline::agent::PiAgentClient;
use pipeline::core::config::PipelineConfig;
use pipeline::execution::{ExecutionEngine, SchedulingStrategy};

/// Run a pipeline with the real Pi agent
async fn run_pipeline_with_real_pi(yaml: &str) -> Result<IntegrationTestResult, Box<dyn std::error::Error>> {
    let config = PipelineConfig::from_yaml(yaml)?;
    let mut pipeline = config.to_pipeline();

    let pi_client = PiAgentClient::new(pipeline::agent::AgentClientConfig::default());

    let start = std::time::Instant::now();
    let engine = ExecutionEngine::new(pi_client, SchedulingStrategy::Sequential, false);
    engine.execute(&mut pipeline).await?;
    let duration = start.elapsed();

    Ok(IntegrationTestResult {
        pipeline,
        duration_ms: duration.as_millis() as u64,
    })
}

/// Result from running an integration test
#[derive(Debug, Clone)]
pub struct IntegrationTestResult {
    pub pipeline: pipeline::core::Pipeline,
    pub duration_ms: u64,
}

impl IntegrationTestResult {
    /// Check if the pipeline completed successfully
    pub fn is_success(&self) -> bool {
        matches!(self.pipeline.state.status, pipeline::core::ExecutionStatus::Completed)
    }

    /// Get the output of a specific step
    pub fn get_step_output(&self, step_id: &str) -> Option<String> {
        self.pipeline.step(step_id).and_then(|s| {
            match &s.state {
                pipeline::core::state::StepState::Completed { output, .. } => Some(output.clone()),
                _ => None,
            }
        })
    }
}

/// Test with real Pi - simple hello prompt
#[tokio::test]
#[ignore]  // Only run with --ignored
async fn test_with_real_pi_hello() {
    let yaml = r#"
name: "Test Pi Hello"

steps:
  - id: "hello"
    name: "Hello"
    prompt: "Say hello"
    termination:
      success_pattern: "hello"
"#;

    let result = run_pipeline_with_real_pi(yaml).await.unwrap();

    assert!(result.is_success(), "Pipeline should complete successfully");

    let output = result.get_step_output("hello")
        .expect("Step 'hello' should have output");

    // Check that output contains something
    assert!(!output.is_empty(), "Output should not be empty");
    println!("Hello output: {}", output);
}

/// Test with real Pi - simple pipeline
#[tokio::test]
#[ignore]
async fn test_with_real_pi_simple_pipeline() {
    let yaml = r#"
name: "Simple Pipeline with Real Pi"

steps:
  - id: "plan"
    name: "Create Plan"
    prompt: |
      Create a brief plan for building a TODO app.
      End your response with exactly: ✅ PLAN COMPLETE
    termination:
      success_pattern: "✅ PLAN COMPLETE"
      on_success: "implement"

  - id: "implement"
    name: "Implement"
    depends_on: ["plan"]
    prompt: |
      Based on this plan:
      {{ steps.plan.output }}

      Write a simple implementation.
      End your response with exactly: ✅ IMPLEMENTATION_DONE
    termination:
      success_pattern: "✅ IMPLEMENTATION_DONE"
"#;

    let result = run_pipeline_with_real_pi(yaml).await.unwrap();

    assert!(result.is_success());

    // Check both steps have output
    let plan_output = result.get_step_output("plan")
        .expect("Step 'plan' should have output");
    let impl_output = result.get_step_output("implement")
        .expect("Step 'implement' should have output");

    assert!(!plan_output.is_empty());
    assert!(!impl_output.is_empty());

    println!("Plan output length: {} chars", plan_output.len());
    println!("Implementation output length: {} chars", impl_output.len());
}

/// Test with real Pi - verify timeout handling
#[tokio::test]
#[ignore]
async fn test_with_real_pi_timeout() {
    use std::time::Duration;

    let yaml = r#"
name: "Test Timeout"

steps:
  - id: "quick"
    name: "Quick Task"
    prompt: "Say hello world"
    timeout_secs: 30
    termination:
      success_pattern: "hello"
"#;

    let config = PipelineConfig::from_yaml(yaml).unwrap();
    let mut pipeline = config.to_pipeline();

    let pi_client = PiAgentClient::new(pipeline::agent::AgentClientConfig::default());

    let start = std::time::Instant::now();
    let engine = ExecutionEngine::new(pi_client, SchedulingStrategy::Sequential, false);
    let result = tokio::time::timeout(Duration::from_secs(60), engine.execute(&mut pipeline)).await;
    let elapsed = start.elapsed();

    // Should complete within timeout (Pi responds reasonably quickly)
    assert!(result.is_ok(), "Pipeline should complete within timeout");
    assert!(elapsed < Duration::from_secs(50), "Should complete in under 50s");

    if let Ok(Ok(())) = result {
        assert!(pipeline.is_complete());
    }
}

/// Test with real Pi - verify error handling
#[tokio::test]
#[ignore]
async fn test_with_real_pi_error_handling() {
    let yaml = r#"
name: "Test Error Handling"

steps:
  - id: "task"
    name: "Task"
    prompt: "Say ERROR to test failure"
    termination:
      success_pattern: "✅ SUCCESS"
      on_failure: "fallback"

  - id: "fallback"
    name: "Fallback"
    depends_on: ["task"]
    prompt: "Handle the error"
    termination:
      success_pattern: "✅ HANDLED"
"#;

    let config = PipelineConfig::from_yaml(yaml).unwrap();
    let mut pipeline = config.to_pipeline();

    let pi_client = PiAgentClient::new(pipeline::agent::AgentClientConfig::default());
    let engine = ExecutionEngine::new(pi_client, SchedulingStrategy::Sequential, false);

    let _ = engine.execute(&mut pipeline).await;

    // Task should fail (no ✅ SUCCESS pattern), but fallback should run
    let task_state = pipeline.step("task").map(|s| &s.state);
    let fallback_state = pipeline.step("fallback").map(|s| &s.state);

    // Fallback should have completed
    assert!(matches!(fallback_state, Some(pipeline::core::StepState::Completed { .. })),
            "Fallback should complete");

    println!("Task state: {:?}", task_state);
    println!("Fallback state: {:?}", fallback_state);
}

/// Test with real Pi - verify variable substitution works
#[tokio::test]
#[ignore]
async fn test_with_real_pi_variable_substitution() {
    let yaml = r#"
name: "Test Variable Substitution"

variables:
  task: "build a simple calculator"
  language: "JavaScript"

steps:
  - id: "task"
    name: "Do Task"
    prompt: |
      I need to {{ task }} using {{ language }}.
      Please write a brief plan.
      End your response with exactly: ✅ DONE
    termination:
      success_pattern: "✅ DONE"
"#;

    let result = run_pipeline_with_real_pi(yaml).await.unwrap();

    assert!(result.is_success());

    let output = result.get_step_output("task")
        .expect("Step should have output");

    assert!(!output.is_empty());
    // Output should mention the variables (at least the task)
    println!("Output mentions calculator: {}", output.to_lowercase().contains("calculator"));
    println!("Output: {}", output);
}

/// Test with real Pi - multi-step workflow
#[tokio::test]
#[ignore]
async fn test_with_real_pi_multi_step() {
    let yaml = r#"
name: "Multi-Step Workflow"

steps:
  - id: "design"
    name: "Design"
    prompt: "Create a brief design for a note-taking app. End with ✅ DESIGN_DONE"
    termination:
      success_pattern: "✅ DESIGN_DONE"
      on_success: "implement"

  - id: "implement"
    name: "Implement"
    depends_on: ["design"]
    prompt: "Based on the design, write a simple implementation. End with ✅ IMPL_DONE"
    termination:
      success_pattern: "✅ IMPL_DONE"
      on_success: "review"

  - id: "review"
    name: "Review"
    depends_on: ["implement"]
    prompt: "Review the implementation. End with ✅ REVIEW_DONE"
    termination:
      success_pattern: "✅ REVIEW_DONE"
"#;

    let result = run_pipeline_with_real_pi(yaml).await.unwrap();

    assert!(result.is_success());

    // All steps should have output
    for step_id in ["design", "implement", "review"] {
        let output = result.get_step_output(step_id)
            .expect(&format!("Step '{}' should have output", step_id));
        assert!(!output.is_empty(), "Step '{}' output should not be empty", step_id);
        println!("{} output: {} chars", step_id, output.len());
    }
}
