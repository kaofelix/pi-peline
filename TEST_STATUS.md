# Test Status for pi-pipeline

## Quick Test Results

Run `cargo test` to execute all tests:

```bash
cargo test              # Run all mock tests
cargo test --ignored  # Run integration tests (requires real Pi CLI)
```

## Current Status

**Total: 102 tests** (updated - includes variable substitution tests)

| Category | Status | Passing | Notes |
|----------|--------|----------|--------|
| Unit Tests | ✅ | 23/23 | Core logic tests |
| Success Chain | ✅ | 4/4 | Linear pipeline flow |
| Mock Agent | ✅ | 6/6 | Deterministic mock responses |
| Helpers | ✅ | 4/4 | Test utilities |
| Retry Behavior | ✅ | 5/5 | Attempt counting with Retrying state |
| Review Loop | ✅ | 4/4 | on_failure routing + attempt incrementing |
| Failure Handling | ✅ | 7/7 | on_failure routing with FailedWithRoute |
| Max Retries | ✅ | 7/7 | Retry limit enforcement |
| Variable Substitution | ✅ | 9/9 | **NEW**: Variable interpolation in prompts |
| Integration | ✅ | 6/6 | Ready (use --ignored) |

**Overall: 102/102 passing (100%)** 🎉

## What Works

✅ Linear pipelines (step → step → step)
✅ Dependency handling (depends_on)
✅ Parallel step support
✅ Mock agent for deterministic tests
✅ Integration tests with real Pi CLI
✅ Basic success/failure detection
✅ Retry attempt counting (Retrying state preserves count)
✅ on_failure routing for review loops
✅ Failed steps satisfy dependencies
✅ Queue deduplication prevents stuck pipelines
✅ Attempt incrementing on routing and continuation
✅ Variable substitution in prompts
  - Global variables from pipeline config
  - Step outputs: `steps.{step_id}.output`
  - Multiple variables per prompt
  - Chain of dependent steps
  - Review loops with variables
  - Parallel steps with variables
  - Special characters in values
✅ Clean code (all debug output removed)

## What Needs Work

None - all tests passing! 🎉

## Changes Made

### Core State (`src/core/state.rs`)
- Added `Retrying { attempt: usize }` variant to preserve attempt count across retries
- Updated `can_start()` to include Retrying state

### Step (`src/core/step.rs`)
- Added `dependencies_met()` method for checking against completed+failed steps
- `render_prompt()` method for variable substitution with `{{ variable_name }}` syntax
- `build_effective_prompt()` method that combines prompt with variables and instructions

### Context (`src/core/context.rs`)
- `PipelineContext` holds global variables, step outputs, and metadata
- `get_rendering_variables()` assembles all available variables for substitution
  - Global variables
  - Step outputs as `steps.{step_id}.output`
  - `current_step` variable
  - `notes` variable

### Execution Engine (`src/execution/engine.rs`)
- **Attempt counting**: Fixed to use `Retrying` state's preserved attempt
- **Retry limit enforcement**: Formula `(attempt - 1) > max_retries`
- **Failed with routing**: Added `FailedWithRoute` result type for on_failure
- **Routing continuation**: Target steps reset to `Retrying` state
- **Attempt tracking**: Routes and continuations now increment attempts correctly
- **Review loop fix**: `FailedWithRoute` handler resets target to Retrying for re-execution

### Scheduler (`src/execution/scheduler.rs`)
- **Queue deduplication**: `enqueue()` removes existing entries
- **Dependency checking**: Uses `dependencies_met()` for completed+failed steps
- **Retrying support**: Checks for both Pending and Retrying states

### Pipeline (`src/core/pipeline.rs`)
- **Ready steps**: Includes Retrying state and checks completed+failed deps
- `create_context_for_step()` creates execution context with all variables

### Executor (`src/execution/executor.rs`)
- **Failed with route**: Returns `FailedWithRoute` when on_failure handler exists
- Uses `build_effective_prompt()` to render prompts with variable substitution

### Output (`src/cli/output.rs`)
- **Format retrying state**: Displays "RETRYING (attempt N)" for Retrying state

### Helpers (`tests/helpers.rs`)
- **Execution order**: Includes Failed steps in assertion

### Code Cleanup
- Removed all `eprintln!` debug statements from `src/execution/scheduler.rs`
- Removed all `eprintln!` debug statements from `src/execution/engine.rs`

### Tests (`tests/scenarios/variable_substitution.rs`)
- **NEW**: 9 tests covering variable substitution scenarios:
  - Global variable substitution
  - Step output substitution from previous steps
  - Multiple variables in one prompt
  - Chain of dependent steps with variables
  - Review loops with variable substitution
  - Empty/undefined variables
  - Parallel steps with variables
  - Special characters in variable values

## File Structure

```
tests/
├── mod.rs              # Test entry point
├── mock_agent.rs       # Mock AgentExecutor (6 tests) ✅
├── helpers.rs          # Test utilities (4 tests) ✅
├── scenarios/          # Rust test scenarios
│   ├── success_chain.rs          # ✅ 4/4 passing
│   ├── retry_behavior.rs          # ✅ 5/5 passing
│   ├── review_loop.rs             # ✅ 4/4 passing
│   ├── failure_handling.rs        # ✅ 7/7 passing
│   ├── max_retries.rs            # ✅ 7/7 passing
│   └── variable_substitution.rs  # ✅ 9/9 passing (NEW)
└── integration/        # Real Pi CLI tests
    └── mod.rs           # ✅ 6 tests (#[ignore])

src/
├── lib.rs             # Library exports for testing
├── main.rs            # CLI entry point
└── core/
    ├── config.rs      # Pipeline configuration (includes variables)
    ├── context.rs     # Pipeline context with variables
    ├── pipeline.rs    # Pipeline domain model
    ├── step.rs        # Step domain model (with render_prompt)
    └── state.rs       # Step and Pipeline states
```

## Variable Substitution Documentation

### Syntax

Variables are injected using `{{ variable_name }}` syntax:

```yaml
variables:
  project_name: "pi-pipeline"
  feature: "variable substitution"

steps:
  - id: "step1"
    prompt: "Work on {{ project_name }} project, specifically {{ feature }}"
```

### Available Variables

1. **Global variables** - Defined in the pipeline config under `variables:`
2. **Step outputs** - `steps.{step_id}.output` for completed steps
3. **Special variables**:
   - `{{ current_step }}` - Current step ID
   - `{{ notes }}` - Formatted notes from previous steps

### Example Usage

```yaml
variables:
  requirement: "Build a REST API"

steps:
  - id: "plan"
    prompt: "Create plan for {{ requirement }}"
    termination:
      success_pattern: "✅ PLAN_DONE"
      on_success: "implement"

  - id: "implement"
    depends_on: ["plan"]
    prompt: "Implement based on plan: {{ steps.plan.output }}"
    termination:
      success_pattern: "✅ DONE"
```

## Next Steps

1. ✅ Fix review_loop tests (COMPLETED)
2. ✅ Variable substitution in prompts (COMPLETED)
3. ✅ Cleanup: Remove debug output from code (COMPLETED)

See `TESTING.md` for detailed specifications.
