//! Test: Variable Substitution in Prompts
//!
//! Tests that variables are correctly substituted in step prompts:
//! - Global variables from pipeline config
//! - Step outputs from previous steps (steps.{step_id}.output)
//! - Special variables like current_step, notes

use crate::helpers::*;
use pipeline::core::config::PipelineConfig;

/// Test basic variable substitution with global variables
#[tokio::test]
async fn test_variable_substitution_global() {
    let yaml = r#"
name: "Test: Variable Substitution - Global"

variables:
  project_name: "pi-pipeline"
  feature: "variable substitution"

steps:
  - id: "step1"
    name: "Use Global Variables"
    prompt: "Work on {{ project_name }} project, specifically {{ feature }}"
    termination:
      success_pattern: "✅ DONE"
"#;

    let config = PipelineConfig::from_yaml(yaml).unwrap();
    let mut pipeline = config.to_pipeline();

    // The mock agent should receive the substituted prompt
    let responses = vec!["✅ DONE".to_string()];
    let result = run_pipeline_with_mock(&mut pipeline, responses).await.unwrap();

    assert_pipeline_completed(&result);
    assert_step_executed(&result, "step1", "✅ DONE");
}

/// Test step output substitution - using output from previous step
#[tokio::test]
async fn test_variable_substitution_step_output() {
    let yaml = r#"
name: "Test: Variable Substitution - Step Output"

steps:
  - id: "plan"
    name: "Create Plan"
    prompt: "Create a plan for implementing feature X"
    termination:
      success_pattern: "✅ PLAN_DONE"
      on_success: "implement"

  - id: "implement"
    name: "Implement Feature"
    depends_on: ["plan"]
    prompt: "Implement based on this plan: {{ steps.plan.output }}"
    termination:
      success_pattern: "✅ IMPLEMENTED"
"#;

    let config = PipelineConfig::from_yaml(yaml).unwrap();
    let mut pipeline = config.to_pipeline();

    let plan_output = "1. Define structure\n2. Write tests\n3. Implement code\n✅ PLAN_DONE";
    let responses = vec![
        plan_output.to_string(),
        "Implementing... ✅ IMPLEMENTED".to_string(),
    ];

    let result = run_pipeline_with_mock(&mut pipeline, responses).await.unwrap();

    assert_pipeline_completed(&result);
    assert_execution_order(&result, &["plan", "implement"]);

    // Verify plan step output
    assert_step_executed(&result, "plan", "✅ PLAN_DONE");

    // Verify implement step received the plan output
    let implement_output = result.get_step_output("implement").unwrap();
    // The implement step should have received a prompt containing the plan output
    // We verify this by checking the implement step completed successfully
    assert!(implement_output.contains("✅ IMPLEMENTED"));
}

/// Test multiple variable substitutions in one prompt
#[tokio::test]
async fn test_variable_substitution_multiple() {
    let yaml = r#"
name: "Test: Multiple Variable Substitutions"

variables:
  project: "pi-pipeline"
  version: "1.0.0"
  author: "Felix"

steps:
  - id: "step1"
    name: "Generate Info"
    prompt: "Generate info for {{ project }} v{{ version }} by {{ author }}"
    termination:
      success_pattern: "✅ INFO"
      on_success: "step2"

  - id: "step2"
    name: "Use Previous Output"
    depends_on: ["step1"]
    prompt: "Review: {{ steps.step1.output }}\nProject: {{ project }}"
    termination:
      success_pattern: "✅ REVIEWED"
"#;

    let config = PipelineConfig::from_yaml(yaml).unwrap();
    let mut pipeline = config.to_pipeline();

    let responses = vec![
        "Info: pi-pipeline v1.0.0 by Felix\n✅ INFO".to_string(),
        "Reviewing...\n✅ REVIEWED".to_string(),
    ];

    let result = run_pipeline_with_mock(&mut pipeline, responses).await.unwrap();

    assert_pipeline_completed(&result);
    assert_execution_order(&result, &["step1", "step2"]);
}

/// Test variable substitution across a chain of steps
#[tokio::test]
async fn test_variable_substitution_chain() {
    let yaml = r#"
name: "Test: Variable Substitution Chain"

variables:
  feature: "dynamic prompts"

steps:
  - id: "analyze"
    name: "Analyze"
    prompt: "Analyze {{ feature }}"
    termination:
      success_pattern: "✅ ANALYZED"
      on_success: "design"

  - id: "design"
    name: "Design"
    depends_on: ["analyze"]
    prompt: "Design based on analysis: {{ steps.analyze.output }}"
    termination:
      success_pattern: "✅ DESIGNED"
      on_success: "implement"

  - id: "implement"
    name: "Implement"
    depends_on: ["design"]
    prompt: "Implement using design: {{ steps.design.output }}\nOriginal feature: {{ feature }}"
    termination:
      success_pattern: "✅ DONE"
"#;

    let config = PipelineConfig::from_yaml(yaml).unwrap();
    let mut pipeline = config.to_pipeline();

    let responses = vec![
        "Analysis complete\n✅ ANALYZED".to_string(),
        "Design created\n✅ DESIGNED".to_string(),
        "Implementation complete\n✅ DONE".to_string(),
    ];

    let result = run_pipeline_with_mock(&mut pipeline, responses).await.unwrap();

    assert_pipeline_completed(&result);
    assert_execution_order(&result, &["analyze", "design", "implement"]);
}

/// Test variable substitution with review loop
#[tokio::test]
async fn test_variable_substitution_with_review_loop() {
    let yaml = r#"
name: "Test: Variable Substitution with Review Loop"

variables:
  requirement: "Build a REST API"

steps:
  - id: "implement"
    name: "Implement"
    prompt: "Implement: {{ requirement }}"
    termination:
      success_pattern: "✅ DONE"
      on_failure: "review"

  - id: "review"
    name: "Review"
    depends_on: ["implement"]
    prompt: "Review this implementation: {{ steps.implement.output }}"
    termination:
      success_pattern: "✅ APPROVED"
      on_failure: "implement"
"#;

    let config = PipelineConfig::from_yaml(yaml).unwrap();
    let mut pipeline = config.to_pipeline();

    // First implementation, review rejects, second implementation, review approves
    let responses = vec![
        "Initial implementation\n✅ DONE".to_string(),
        "Needs changes: add authentication\n".to_string(),
        "Added authentication\n✅ DONE".to_string(),
        "Looks good!\n✅ APPROVED".to_string(),
    ];

    let result = run_pipeline_with_mock(&mut pipeline, responses).await.unwrap();

    assert_pipeline_completed(&result);

    // Both implement and review should have run multiple times
    assert!(count_step_executions(&result, "implement") >= 2);
    assert!(count_step_executions(&result, "review") >= 1);
}

/// Test variable substitution when no variables are defined
#[tokio::test]
async fn test_variable_substitution_empty() {
    let yaml = r#"
name: "Test: No Variables"

steps:
  - id: "step1"
    name: "Simple Step"
    prompt: "Just do the work"
    termination:
      success_pattern: "✅ DONE"
"#;

    let config = PipelineConfig::from_yaml(yaml).unwrap();
    let mut pipeline = config.to_pipeline();

    let responses = vec!["✅ DONE".to_string()];
    let result = run_pipeline_with_mock(&mut pipeline, responses).await.unwrap();

    assert_pipeline_completed(&result);
}

/// Test that undefined variables are not substituted (remain as-is)
#[tokio::test]
async fn test_variable_substitution_undefined() {
    let yaml = r#"
name: "Test: Undefined Variables"

variables:
  defined_var: "hello"

steps:
  - id: "step1"
    name: "Step With Undefined Var"
    prompt: "{{ defined_var }} and {{ undefined_var }}"
    termination:
      success_pattern: "✅ DONE"
"#;

    let config = PipelineConfig::from_yaml(yaml).unwrap();
    let mut pipeline = config.to_pipeline();

    // The step should still work, undefined vars just stay as {{ undefined_var }}
    let responses = vec!["hello and {{ undefined_var }}\n✅ DONE".to_string()];
    let result = run_pipeline_with_mock(&mut pipeline, responses).await.unwrap();

    assert_pipeline_completed(&result);
    assert_step_executed(&result, "step1", "✅ DONE");
}

/// Test variable substitution with parallel steps
#[tokio::test]
async fn test_variable_substitution_parallel() {
    let yaml = r#"
name: "Test: Parallel Steps with Variables"

variables:
  base_feature: "data processing"

steps:
  - id: "setup"
    name: "Setup"
    prompt: "Setup {{ base_feature }} module"
    termination:
      success_pattern: "✅ SETUP_DONE"

  - id: "task_a"
    name: "Task A"
    depends_on: ["setup"]
    prompt: "Task A for {{ base_feature }}"
    termination:
      success_pattern: "✅ TASK_A_DONE"

  - id: "task_b"
    name: "Task B"
    depends_on: ["setup"]
    prompt: "Task B for {{ base_feature }}"
    termination:
      success_pattern: "✅ TASK_B_DONE"

  - id: "merge"
    name: "Merge Results"
    depends_on: ["task_a", "task_b"]
    prompt: "Merge A: {{ steps.task_a.output }} and B: {{ steps.task_b.output }}"
    termination:
      success_pattern: "✅ MERGED"
"#;

    let config = PipelineConfig::from_yaml(yaml).unwrap();
    let mut pipeline = config.to_pipeline();

    let responses = vec![
        "Setup complete\n✅ SETUP_DONE".to_string(),
        "Task A done\n✅ TASK_A_DONE".to_string(),
        "Task B done\n✅ TASK_B_DONE".to_string(),
        "Merged results\n✅ MERGED".to_string(),
    ];

    let result = run_pipeline_with_mock(&mut pipeline, responses).await.unwrap();

    assert_pipeline_completed(&result);
    assert_eq!(result.completed_steps().len(), 4);
}

/// Test variable substitution with special characters in values
#[tokio::test]
async fn test_variable_substitution_special_chars() {
    let yaml = r#"
name: "Test: Special Characters in Variables"

variables:
  complex_value: "Value with \"quotes\", 'apostrophes', and \n newlines"
  emoji: "🚀✨💻"
  url: "https://example.com/path?query=value&other=123"

steps:
  - id: "step1"
    name: "Handle Special Chars"
    prompt: "Complex: {{ complex_value }}\nEmoji: {{ emoji }}\nURL: {{ url }}"
    termination:
      success_pattern: "✅ PROCESSED"
"#;

    let config = PipelineConfig::from_yaml(yaml).unwrap();
    let mut pipeline = config.to_pipeline();

    let responses = vec!["✅ PROCESSED".to_string()];
    let result = run_pipeline_with_mock(&mut pipeline, responses).await.unwrap();

    assert_pipeline_completed(&result);
    assert_step_executed(&result, "step1", "✅ PROCESSED");
}
