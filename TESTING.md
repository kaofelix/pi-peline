# Testing Guide

## Overview

Fast deterministic tests using mock agents, plus integration tests with real Pi CLI.

```bash
cargo test              # Mock tests (102 tests)
cargo test --ignored    # Integration tests (requires Pi CLI)
```

## Architecture

```
tests/
├── mock_agent.rs       # Deterministic AgentExecutor
├── helpers.rs          # Reusable test utilities
├── scenarios/          # Behavioral tests
│   ├── success_chain.rs
│   ├── retry_behavior.rs
│   ├── review_loop.rs
│   ├── failure_handling.rs
│   ├── max_retries.rs
│   └── variable_substitution.rs
└── integration/mod.rs  # Real Pi CLI tests
```

## Mock Agent

Deterministic responses without subprocess overhead.

```rust
use pipeline::AgentExecutor;

let responses = vec!["First response".to_string(), "✅ DONE".to_string()];
let agent = MockAgent::new(responses);
```

## Test Helpers

Quick assertions for pipeline results.

```rust
use crate::helpers::*;

// Run pipeline with mock responses
let result = run_pipeline_with_mock(&mut pipeline, responses).await?;

// Assertions
assert_pipeline_completed(&result);
assert_step_executed(&result, "step1", "✅ DONE");
assert_execution_order(&result, &["step1", "step2", "step3"]);
let attempts = count_step_executions(&result, "step1");
```

## Writing Tests

```rust
#[tokio::test]
async fn test_success_chain() {
    let yaml = r#"
name: "Test Pipeline"
steps:
  - id: "plan"
    prompt: "Create plan"
    termination:
      success_pattern: "✅ PLAN COMPLETE"
      on_success: "implement"
  - id: "implement"
    depends_on: ["plan"]
    prompt: "Implement"
    termination:
      success_pattern: "✅ DONE"
"#;

    let config = PipelineConfig::from_yaml(yaml).unwrap();
    let mut pipeline = config.to_pipeline();

    let responses = vec!["✅ PLAN COMPLETE".to_string(), "✅ DONE".to_string()];
    let result = run_pipeline_with_mock(&mut pipeline, responses).await.unwrap();

    assert_pipeline_completed(&result);
    assert_execution_order(&result, &["plan", "implement"]);
}
```

## Variable Substitution

Tests for `{{ variable_name }}` syntax in prompts.

**Available variables:**
- Global: `variables:` section in config
- Step outputs: `steps.{step_id}.output`
- Special: `{{ current_step }}`, `{{ notes }}`

```yaml
variables:
  project: "pi-pipeline"

steps:
  - id: "plan"
    prompt: "Create plan for {{ project }}"
    termination:
      success_pattern: "✅ DONE"
      on_success: "implement"

  - id: "implement"
    depends_on: ["plan"]
    prompt: "Implement based on: {{ steps.plan.output }}"
```

## Scenario Coverage

| Scenario | What It Tests |
|----------|---------------|
| success_chain | Linear pipeline flow |
| retry_behavior | Attempt counting with Retrying state |
| review_loop | on_failure routing + attempt incrementing |
| failure_handling | FailedWithRoute behavior |
| max_retries | Retry limit enforcement |
| variable_substitution | Variable interpolation in prompts |

## Key Implementation Details

### StepState Variants

- `Pending` - Initial state
- `Retrying { attempt }` - Waiting to retry (preserves count)
- `Running { started_at, attempt }` - Currently executing
- `Completed { output, attempts, ... }` - Finished successfully
- `Failed { error, attempts, ... }` - Retries exhausted
- `Skipped { reason }` - Not executed
- `Blocked { reason, blocked_at }` - Waiting on external condition

### Retry Counting

1. Step starts as `Pending` (attempt=1)
2. On retry, becomes `Retrying { attempt: 1 }`
3. Next execution preserves attempt count
4. Formula: `(attempt - 1) > max_retries`

### on_failure Routing

Failed steps with `on_failure` targets:
1. Step marked `Failed`
2. Pipeline status reset to `Running`
3. Target step enqueued as `Retrying`
4. Failed steps satisfy dependencies

## Running Specific Tests

```bash
cargo test test_success_chain
cargo test scenarios::success_chain
cargo test -- --nocapture           # Show output
cargo test -- --test-threads=1     # Sequential execution
```
