# pi-peline Roadmap

This document provides detailed descriptions of planned features for pi-peline.

---

## Human-in-the-Loop Checkpoints

### Overview
Allow pipeline steps that pause execution and wait for manual human approval or rejection before continuing. This enables users to review agent outputs and provide feedback, creating interactive workflows.

### Motivating Examples

1. **Plan Review**
   - Agent generates an implementation plan
   - Pipeline pauses and presents the plan to the user
   - User reviews and either approves (continue) or provides feedback (route back)

2. **Code Review**
   - Agent implements a feature
   - Pipeline pauses for code review
   - User can approve, request changes, or suggest modifications

3. **Output Verification**
   - Agent generates documentation or configuration
   - User verifies correctness before proceeding

### Proposed YAML Syntax

```yaml
steps:
  - id: "planning"
    name: "Create Implementation Plan"
    prompt: "Create a detailed implementation plan for {{ feature_name }}"
    termination:
      success_pattern: "✅ PLAN_COMPLETE"
      on_success: "review_plan"

  - id: "review_plan"
    name: "Review Plan"
    type: "checkpoint"  # New field indicating human-in-the-loop step
    depends_on: ["planning"]
    message: |
      Please review the following plan:
      {{ steps.planning.output }}

      Do you approve this plan? (y/n)
    on_approve: "implement"      # Continue on approval
    on_reject: "planning"         # Route back on rejection
    capture_feedback: true        # Capture user's feedback/comments
```

### Implementation Notes

**CLI Behavior:**
- Pause execution at checkpoint steps
- Display the previous step's output with context
- Prompt user for input (y/n or detailed feedback)
- Store feedback in context if `capture_feedback: true`
- Route to `on_approve` or `on_reject` step accordingly

**Context Integration:**
- Captured feedback should be available to subsequent steps via `{{ checkpoint.feedback }}` or similar
- Could also support multi-choice options, not just y/n

**Web UI Consideration:**
- Web UI should display checkpoint steps differently (e.g., yellow status)
- Provide clear approve/reject buttons and comment input

### Open Questions

1. Should checkpoints be able to present arbitrary content (not just previous step output)?
2. Should feedback be captured as plain text or structured data?
3. Should there be timeout for checkpoints (auto-reject after X minutes)?
4. Should checkpoints support multiple reviewers/decision-makers?
5. How should checkpoint state persist across pipeline runs?

---

## Shell Script Quality Gates

### Overview
Enable pipeline steps that execute shell commands (scripts, tests, linters) as quality gates. On failure, route to remediation steps with command output for automated fixes.

### Motivating Examples

1. **Test Gate**
   - Agent implements a feature
   - Run `npm test` or `cargo test`
   - If tests fail, route to a "fix tests" step with the failure output

2. **Lint Gate**
   - Agent generates code
   - Run `eslint .` or `ruff check .`
   - Route back with lint errors if any

3. **Build Gate**
   - Verify the project builds successfully
   - `npm run build` or `cargo build`
   - Route back on build failure

4. **Custom Scripts**
   - Run any custom validation script
   - Example: security scan, smoke tests, integration tests

### Proposed YAML Syntax

```yaml
steps:
  - id: "implement"
    name: "Implement Feature"
    prompt: "Implement the feature according to the plan"
    termination:
      success_pattern: "✅ IMPLEMENTATION_DONE"
      on_success: "test"

  - id: "test"
    name: "Run Tests"
    type: "shell"  # New field indicating shell execution step
    depends_on: ["implement"]
    command: "npm test"
    env:
      NODE_ENV: "test"
    working_dir: "./"
    timeout_secs: 60
    on_success: "deploy"
    on_failure: "fix_tests"
    capture_output: true  # Make output available to next steps

  - id: "fix_tests"
    name: "Fix Failed Tests"
    depends_on: ["test"]
    prompt: |
      The tests failed with the following output:
      {{ steps.test.output }}

      Fix the failing tests and code to make them pass.
    termination:
      success_pattern: "✅ TESTS_FIXED"
      on_success: "test"  # Re-run tests after fix
```

### Implementation Notes

**Command Execution:**
- Execute commands in a subprocess with optional timeout
- Capture stdout and stderr (combined or separate)
- Support environment variables via `env:` field
- Support working directory specification

**Exit Code Handling:**
- Exit code 0 → success
- Non-zero exit code → failure
- Configurable success criteria (e.g., treat certain non-zero codes as success)

**Output Capture:**
- Make command output available via `{{ steps.<step_id>.output }}`
- Optionally support `{{ steps.<step_id>.stdout }}` and `{{ steps.<step_id>.stderr }}` separately
- Support truncating large outputs (configurable max lines/bytes)

**Integration with Agent Steps:**
- Shell steps should work seamlessly in dependency chains
- Agent steps can read shell output and use it for fixes
- Enable loops: agent → shell (fail) → agent (fix) → shell (retry)

### Advanced Features

**Multi-command steps:**
```yaml
type: "shell"
commands:
  - "npm install"
  - "npm run lint"
  - "npm test"
stop_on_failure: true  # Stop after first failure
```

**Conditional execution:**
```yaml
type: "shell"
command: "npm run {{ test_command }}"
condition: "{{ run_tests == true }}"
```

**Platform-specific commands:**
```yaml
type: "shell"
command:
  windows: "npm run test:windows"
  unix: "npm test"
```

### Open Questions

1. Should shell commands be run with a specific user/permissions?
2. How to handle interactive commands that require user input?
3. Should there be a whitelist/blacklist of allowed commands?
4. How to handle commands that hang indefinitely (timeout behavior)?
5. Should output be streamed in real-time or captured all at once?
6. How to handle extremely large outputs (log files, etc.)?

---

## Web UI

### Overview
A web-based interface for creating, managing, and executing pipelines. Provides visual pipeline editing, execution monitoring, and historical analysis.

### Planned Features

- **Pipeline Editor** - Visual YAML editor with syntax highlighting and validation
- **Execution Dashboard** - Real-time pipeline execution status and logs
- **History View** - Browse past executions with filtering and comparison
- **Variable Management** - Define and manage global variables
- **Checkpoint UI** - Interactive approval/rejection interface for human-in-the-loop steps
- **Shell Output Viewer** - Display shell command outputs with syntax highlighting

### Tech Stack Considerations

- Rust web framework (Actix-web, Axum) for backend
- Frontend framework (React, Vue, or vanilla with htmx)
- WebSocket support for real-time updates
- SQLite for persistence (existing)

---

## Future Ideas (Not Yet Prioritized)

- **Pipeline Templates** - Pre-built pipeline templates for common workflows
- **Step Composition/Reusability** - Define reusable step libraries
- **Conditional Step Execution** - Skip steps based on conditions
- **Loop/Repeat Patterns** - Repeat steps N times or until condition met
- **Variable Inheritance** - Pipeline-level vs step-level variable scoping
- **Secret/Environment Variable Support** - Secure handling of sensitive values
- **Distributed Execution** - Run steps across multiple machines
- **Parallel Execution** - Run independent steps concurrently
- **Multiple Agent Support** - Use Claude Code, OpenAI, etc., in same pipeline
