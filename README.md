# pi-peline

> Warning: this is a 100% vibe coded experiment, not recommended for serious usage. The README might be misleading and mention incomplete or unimplemented features

An AI agent orchestration tool for executing multi-step workflows through the Pi CLI coding agent.

## What is pi-peline?

pi-peline (Pi + Pipeline) is a tool for defining and executing multi-step AI agent workflows. Inspired by CI/CD tools like GitHub Actions, but designed specifically for orchestrating the Pi CLI coding agent rather than deploying software.

You define a pipeline structure where each step executes a prompt through the Pi CLI agent. Steps can depend on each other, branch based on outputs, or loop back for revision. It's a way to experiment with telling Pi to execute different prompts over a codebase or any task.

**Note**: pi-peline currently works with the Pi CLI coding agent. In the future, it will support other AI agents like Claude Code.

## Key Features

- **YAML-based pipeline definitions** - Declarative configuration
- **Step dependencies** - Define which steps depend on others
- **Termination promises** - The agent signals completion by printing a specific string
- **Continuation promises** - The agent can request more work or route to different steps
- **Review loops** - Implementation steps can route back for revision based on feedback
- **Execution history** - All runs are persisted to SQLite
- **Local execution** - Runs locally

## Use Cases

- **Feature development workflow** - Plan → Implement → Review → Deploy
- **Code refactoring** - Analyze → Plan → Refactor → Verify
- **Documentation generation** - Analyze code → Generate docs → Review
- **Testing workflows** - Generate tests → Execute → Report
- **Multi-step code analysis** - Break complex tasks into coordinated agent steps

## Installation

```bash
cargo install --path .
```

## Quick Start

Create a simple pipeline in `pipeline.yaml`:

```yaml
name: "Feature Development Pipeline"
version: "1.0"

variables:
  feature_name: "user authentication"

  # File variable - validates README.md exists
  readme:
    path: "README.md"
    validate_exists: true

steps:
  - id: "planning"
    name: "Create Implementation Plan"
    prompt: |
      Create a detailed implementation plan for {{ feature_name }}.

      Reference this README:
      {{ readme }}
    termination:
      success_pattern: "✅ PLAN COMPLETE"
      on_success: "implementation"

  - id: "implementation"
    name: "Implement Feature"
    depends_on: ["planning"]
    prompt: |
      Implement the feature based on this plan:
      {{ steps.planning.output }}
    termination:
      success_pattern: "✅ IMPLEMENTATION_DONE"
      on_success: "review"
    continuation:
      pattern: "🔄 CONTINUE"
      action: "retry"

  - id: "review"
    name: "Review Implementation"
    depends_on: ["implementation"]
    prompt: |
      Review this implementation:
      {{ steps.implementation.output }}

      If issues found, specify what's missing.
    termination:
      success_pattern: "✅ APPROVED"
      on_success: "deploy"
      on_failure: "implementation"
    continuation:
      pattern: "🔄 NEEDS_REVISION"
      action: "route"
      target: "implementation"
      carry_notes: true

  - id: "deploy"
    name: "Prepare Deployment"
    depends_on: ["review"]
    prompt: "Prepare deployment checklist for approved implementation"
    termination:
      success_pattern: "✅ DEPLOYED"
```

Run the pipeline:

```bash
pi-peline run --file pipeline.yaml
```

## CLI Commands

### Run a Pipeline

```bash
pi-peline run --file pipeline.yaml

# With variable overrides
pi-peline run --file pipeline.yaml --variable feature_name="new feature"

# With streaming output
pi-peline run --file pipeline.yaml --stream

# Skip history
pi-peline run --file pipeline.yaml --no-history
```

### Validate a Pipeline

```bash
pi-peline validate --file pipeline.yaml
```

### List Pipelines

```bash
pi-peline list

# With execution counts
pi-peline list --with-counts

# JSON output
pi-peline list --json
```

### Show History

```bash
pi-peline history

# For a specific pipeline
pi-peline history --pipeline "Feature Development Pipeline"

# With details
pi-peline history --verbose

# JSON output
pi-peline history --json
```

## Pipeline Configuration Reference

### Top-level Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | Yes | Pipeline name |
| `version` | string | No | Pipeline version |
| `variables` | map | No | Global variables available to all steps |
| `max_retries` | number | No | Default max retries per step |
| `default_timeout_secs` | number | No | Default timeout per step |
| `steps` | array | Yes | Array of step definitions |

### Step Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | string | Yes | Unique step identifier |
| `name` | string | Yes | Human-readable step name |
| `description` | string | No | Step description |
| `prompt` | string | Yes | The prompt template for the agent |
| `depends_on` | array | No | List of step IDs this step depends on |
| `termination` | object | No | Termination condition |
| `continuation` | object | No | Continuation condition |
| `max_retries` | number | No | Override default max retries |
| `timeout_secs` | number | No | Override default timeout |
| `allow_parallel` | boolean | No | Allow parallel execution (default: false) |

### Termination Condition

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `success_pattern` | string | Yes | String that signals successful completion |
| `on_success` | string | No | Step ID to execute on success (null = end) |
| `on_failure` | string | No | Step ID to execute on failure |
| `use_regex` | boolean | No | Use regex pattern matching (default: false) |

### Continuation Condition

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `pattern` | string | Yes | String that signals continuation needed |
| `action` | enum | Yes | "retry" or "route" |
| `target` | string | No* | Target step when action is "route" |
| `carry_notes` | boolean | No | Pass notes when routing |
| `use_regex` | boolean | No | Use regex pattern matching (default: false) |

* Required when action is "route"

### Variables

Variables can be defined in two formats:

**Simple string variable:**
```yaml
variables:
  feature_name: "user authentication"
```

**File variable with validation:**
```yaml
variables:
  readme:
    path: "README.md"
    validate_exists: true  # Optional, defaults to false
```

When a file variable is used in a prompt, it is expanded as `@path` so the Pi CLI agent can read the file directly:

```yaml
steps:
  - id: "analyze"
    prompt: |
      Analyze this README:
      {{ readme }}
    # Expands to: "Analyze this README: @README.md"
```

**File variable fields:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `path` | string | Yes | Path to the file |
| `validate_exists` | boolean | No | If true, validation fails if file doesn't exist (default: false) |

Use `validate_exists: true` for input files that must exist, and `validate_exists: false` for output files or optional files.

## How It Works

1. **Pipeline Loading**: The YAML file is parsed and validated
2. **Graph Construction**: Steps are organized into a DAG based on dependencies
3. **Execution**: The engine executes steps in dependency order:
   - Waits for dependencies to complete
   - Injects termination/continuation instructions into the prompt
   - Executes the Pi CLI agent
   - Watches for termination pattern
   - Routes to next step based on success/failure/continuation

### Prompt Injection

The agent sees an enhanced prompt that includes instructions:

```
[YOUR PROMPT]

--- IMPORTANT: When you complete this task successfully, print exactly: ✅ DONE
If you need more work on this task, print exactly: 🔄 CONTINUE
```

### Review Loop Pattern

A common pattern is a review loop where implementation and review steps iterate:

1. **Implementation step** creates something
2. **Review step** evaluates it
3. If approved → Continue to next step
4. If rejected → Back to Implementation with notes

```yaml
steps:
  - id: "implement"
    prompt: "Create X"
    termination:
      success_pattern: "✅ DONE"
      on_success: "review"

  - id: "review"
    prompt: "Review the implementation"
    termination:
      success_pattern: "✅ APPROVED"
      on_success: "deploy"
      on_failure: "implement"
    continuation:
      pattern: "🔄 NEEDS_REVISION"
      action: "route"
      target: "implement"
      carry_notes: true
```

### Variable Substitution

Variables are injected using `{{ variable_name }}` syntax:

```yaml
variables:
  feature_name: "authentication"
  project_dir: "./src"

steps:
  - id: "analyze"
    prompt: |
      Analyze {{ feature_name }} in {{ project_dir }}
```

Previous step outputs are also available:

```yaml
steps:
  - id: "plan"
    prompt: "Create a plan"

  - id: "implement"
    depends_on: ["plan"]
    prompt: |
      Implement based on:
      {{ steps.plan.output }}
```

## Development

### Build

```bash
cargo build
```

### Run Tests

```bash
cargo test
```

### Run Example

```bash
cargo run -- run --file examples/pipeline.yaml
```

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                      CLI                               │
│  (pi-peline run, validate, list, history)               │
└────────────────────┬────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────┐
│                 Execution Engine                        │
│  - Scheduler (sequential/parallel)                     │
│  - Step Executor                                      │
│  - Event Handlers                                     │
└────────────────────┬────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────┐
│              Pi Agent Client (Subprocess)               │
│  - Spawns `pi --mode text --print --no-session`      │
│  - Captures stdout as response                        │
│  - Handles timeouts and errors                         │
└────────────────────┬────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────┐
│                 Pi CLI (subprocess)                    │
│  - Executes prompts                                   │
│  - Uses tools (read, write, edit, bash)              │
│  - Returns formatted response                         │
└─────────────────────────────────────────────────────────┘
```

## Roadmap

- [x] Pi CLI agent integration
- [x] Context file support (read files from disk)
- [ ] Human-in-the-loop checkpoints
- [ ] Shell script quality gates
- [ ] Web UI

## Configuration

### Pi Binary Path

pi-peline uses the `pi` CLI agent to execute prompts. By default, it assumes `pi` is on your PATH. To use a custom path:

Set the `PI_BINARY_PATH` environment variable, or the `endpoint` field will be used for this in a future release.

```bash
# Use pi from a specific location
export PATH="/custom/path:$PATH"

# Or run with full path
ln -s /path/to/pi /usr/local/bin/pi
```

### Requirements

- `pi` CLI must be installed (see [pi-coding-agent](https://github.com/badlogic/pi-mono))
- `pi` must be accessible on PATH or via `PI_BINARY_PATH`

## License

MIT
