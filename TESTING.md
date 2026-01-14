# Testing Strategy for pi-pipeline

This document outlines the comprehensive testing strategy for pi-pipeline, covering unit tests, mock agents, integration tests, and various testing scenarios.

**For current test status, see [TEST_STATUS.md](TEST_STATUS.md)**

---

## Table of Contents

1. [Overview](#overview)
2. [Testing Layers](#testing-layers)
3. [File Structure](#file-structure)
4. [Running Tests](#running-tests)

---

## Overview

The goal is to create a robust testing framework that:
- **Fast**: Mock agents for deterministic, quick unit tests
- **Real**: Integration tests with actual Pi CLI subprocess
- **Comprehensive**: Cover all pipeline behaviors (success, retry, branching, etc.)
- **Maintainable**: Easy to add new test scenarios
- **Actionable**: Clear failure messages, visual test output

---

## Testing Layers

### 1. Mock Agent

**Location:** `tests/mock_agent.rs`

Deterministic agent for fast unit tests without subprocess overhead.

```rust
use pipeline::AgentExecutor;

let responses = vec!["First response".to_string(), "Second ✅ DONE".to_string()];
let agent = MockAgent::new(responses);
```

**Features:**
- Predefined responses
- Optional delay simulation
- Attempt/response tracking
- No network or subprocess calls

### 2. Test Helpers

**Location:** `tests/helpers.rs`

Reusable utilities for writing tests quickly.

```rust
use crate::helpers::*;

// Run pipeline with mock responses
let result = run_pipeline_with_mock(&mut pipeline, responses).await?;

// Assert pipeline completed
assert_pipeline_completed(&result);

// Assert specific step executed
assert_step_executed(&result, "step1", "✅ DONE");

// Assert execution order (includes failed steps)
assert_execution_order(&result, &["step1", "step2", "step3"]);

// Count step executions (attempts)
let attempts = count_step_executions(&result, "step1");
```

### 3. Scenario Tests

**Location:** `tests/scenarios/*.rs`

Define test scenarios for different pipeline behaviors:

- `success_chain.rs` - ✅ Linear pipeline execution (4/4 passing)
- `retry_behavior.rs` - ✅ Retry with continuation patterns (5/5 passing)
- `review_loop.rs` - ⚠️ Back-and-forth routing (0/4 passing - needs test redesign)
- `failure_handling.rs` - ✅ on_failure routing (7/7 passing)
- `max_retries.rs` - ✅ Retry limit enforcement (7/7 passing)

### 4. Variable Substitution Tests

**Location:** `tests/scenarios/variable_substitution.rs`

Tests for variable interpolation in step prompts.

**Features tested:**
- Global variable substitution
- Step output substitution: `steps.{step_id}.output`
- Multiple variables in one prompt
- Chain of dependent steps with variables
- Review loops with variable substitution
- Parallel steps with variables
- Special characters in variable values

```rust
// Test variable substitution
let yaml = r#"
variables:
  project: "pi-pipeline"
  feature: "variable substitution"

steps:
  - id: "step1"
    prompt: "Work on {{ project }} project, specifically {{ feature }}"
    termination:
      success_pattern: "✅ DONE"
"#;

// Step outputs are available as variables
let yaml = r#"
steps:
  - id: "plan"
    prompt: "Create plan"
    termination:
      success_pattern: "✅ PLAN_DONE"
      on_success: "implement"

  - id: "implement"
    depends_on: ["plan"]
    prompt: "Implement based on plan: {{ steps.plan.output }}"
    termination:
      success_pattern: "✅ DONE"
"#;
```

**Variable Syntax:** `{{ variable_name }}`

**Available Variables:**
- Global variables (defined in `variables:` section)
- Step outputs: `steps.{step_id}.output`
- Special: `{{ current_step }}`, `{{ notes }}`

### 5. Integration Tests

**Location:** `tests/integration/mod.rs`

Tests with real Pi CLI subprocess.

```bash
# Run integration tests
cargo test --ignored
```

Tagged with `#[ignore]` to skip in regular test runs.

---

## File Structure

```
tests/
├── mod.rs              # Test module entry
├── mock_agent.rs       # Mock AgentExecutor
├── helpers.rs          # Test utilities
├── scenarios/          # Rust test scenarios
│   ├── success_chain.rs
│   ├── retry_behavior.rs
│   ├── review_loop.rs
│   ├── failure_handling.rs
│   ├── max_retries.rs
│   └── variable_substitution.rs
└── integration/        # Real Pi CLI tests
    └── mod.rs

src/
└── lib.rs             # Library exports
```

---

## Running Tests

### All tests (mock only)
```bash
cargo test
```

### Integration tests (real Pi)
```bash
cargo test --ignored
```

### Specific test suite
```bash
cargo test scenarios::success_chain
cargo test test_review_loop
```

### Specific test
```bash
cargo test test_success_chain
```

### With output
```bash
cargo test -- --nocapture
```

---

## Example Test

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

    let responses = vec![
        "✅ PLAN COMPLETE".to_string(),
        "✅ DONE".to_string(),
    ];

    let result = run_pipeline_with_mock(&mut pipeline, responses).await.unwrap();

    assert_pipeline_completed(&result);
    assert_execution_order(&result, &["plan", "implement"]);
}
```

## Implementation Notes

### StepState Variants

- `Pending` - Initial state, waiting to execute
- `Retrying { attempt: usize }` - Waiting to retry (preserves attempt count)
- `Running { started_at, attempt }` - Currently executing
- `Completed { output, attempts, started_at, completed_at }` - Finished successfully
- `Failed { error, attempts, last_started_at, failed_at }` - Failed (retries exhausted)
- `Skipped { reason }` - Not executed (conditional)
- `Blocked { reason, blocked_at }` - Waiting on external condition

### Retry Counting

Retry counting uses the `Retrying` state to preserve attempt count:

1. Step starts as `Pending` (attempt=1)
2. On continuation/retry, step becomes `Retrying { attempt: 1 }`
3. On next execution, attempt is NOT incremented (preserves count)
4. Result determines final state (Completed/Failed/Continue)

This avoids off-by-one errors in retry limit enforcement.

### on_failure Routing

When a step fails and has an `on_failure` target:

1. Step is marked as `Failed` with error message
2. Pipeline status is temporarily reset to `Running` (not marked as failed)
3. Target step is enqueued for execution
4. Failed steps satisfy dependencies for subsequent steps

---

**See [TEST_STATUS.md](TEST_STATUS.md) for current test results.**
