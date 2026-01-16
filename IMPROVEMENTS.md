# Improvements

Track technical debt, bugs, and future improvements for the pi-peline project.

---

## Architecture & Code Quality

### Redundant Timeout Configuration

**Issue:** There are 3 timeout defaults in the codebase that are all set to the same value:

1. `AgentClientConfig::default().timeout_secs` (src/agent/client.rs)
2. `StepDefaults::default().timeout_secs` (src/core/step.rs)
3. `Pipeline::from_config()` fallback (src/core/pipeline.rs)

**Current Behavior:**
- `AgentClientConfig` timeout is passed to `PiSubprocessClient`
- `StepExecutor` wraps the agent call in its own timeout using `step.timeout_secs`
- This creates a double timeout situation where both apply

**Root Cause:**
- The agent config timeout is passed down to the subprocess client
- The step executor applies its own timeout on top
- The subprocess timeout is effectively redundant since the executor timeout fires first

**Proposed Fix:**
- Keep `StepDefaults::default().timeout_secs` as the single source of truth
- Remove `timeout_secs` from `AgentClientConfig` (subprocess doesn't need its own timeout)
- `PiSubprocessClient` should not enforce a timeout - let the caller (StepExecutor) handle it
- Update all references to use the step timeout

**Files to Modify:**
- `src/agent/client.rs` - Remove timeout_secs from AgentClientConfig
- `src/agent/subprocess_client.rs` - Remove timeout enforcement
- `src/agent/mod.rs` - Update PiAgentClient::new() to not pass timeout
- Documentation update

**Priority:** Low (works as-is, just confusing)

---

## Fixed Issues

### Smoke Tests Outdated

**Issue:** Smoke tests in `tests/smoke_test.rs` fail with "No termination pattern found"

**Root Cause:** The tests use `PiAgentClient.execute()` which calls the non-streaming `subprocess_client.execute()` (text mode), not the streaming JSON mode. The termination patterns aren't found because the output is in text mode, not parsed JSON.

**Status:** ✅ Fixed

**Solution:** Replaced Rust-based smoke tests with bash script `scripts/smoke_test.sh` that:
- Runs the actual binary with real Pi agent
- Tests core functionality (run, validate, help, variables, streaming)
- Validates output with simple grep patterns
- Fast and catches regressions

**Date Fixed:** 2026-01-16

---

## CLI & User Experience

### Binary Name Inconsistency

**Issue:** The binary is named `pipeline` but the project is called `pi-peline`. Commands reference `pi-peline` but the actual binary is `pipeline`.

**Current Behavior:**
```bash
cargo build --release
# Creates: ./target/release/pipeline (not pi-peline)
```

**Proposed Fix:** Update `Cargo.toml` to set the correct binary name:
```toml
[[bin]]
name = "pi-peline"
path = "src/main.rs"
```

**Priority:** Low (cosmetic, causes confusion)

---

### Better Error Messages for Missing Database Directory

**Issue:** When the SQLite database directory doesn't exist, users get a cryptic error:
```
Error: Failed to connect to database
Caused by:
    error returned from database: (code: 14) unable to open database file
```

**Current Behavior:** Error doesn't explain which directory is missing or how to fix it.

**Proposed Fix:** Add a more helpful error message:
```
Error: Failed to connect to database

The database directory does not exist. To fix this, run:
  mkdir -p ~/Library/Application\ Support/pipeline

Or use --no-history to skip database persistence.
```

**Priority:** Medium (affects first-time users)

---

## Testing

### Integration Tests for Timeout Behavior

**Issue:** No tests verify that timeouts work correctly across the 3-layer timeout configuration.

**Proposed Fix:** Add tests to verify:
- Step timeout correctly terminates long-running steps
- Subprocess doesn't enforce its own timeout (after fix)
- Timeout error messages are clear

**Priority:** Low

---

## Documentation

### Document Timeout Configuration

**Issue:** README doesn't explain how timeouts work or how to configure them.

**Proposed Fix:** Add section on:
- How to configure timeouts per-step in YAML
- Default timeout values
- Why long-running tasks may need longer timeouts

**Priority:** Medium

---

## Performance

### State File Inefficiency

**Issue:** Agent reads and writes to `.observability-state.md` on every step. This could be inefficient for large projects.

**Current Behavior:** Agent explicitly uses `read` and `write` tools to manage state file.

**Proposed Fix:** Consider:
- In-memory step outputs (already available via `steps.{step_id}.output`)
- Only write state file when explicitly needed for persistence
- Or keep as-is - the file is useful for debugging

**Priority:** Low (works as-is, may not need optimization)

---

## Observability

### Tool Call ID Validation Not Full

**Issue:** ToolExecutionStart/End events include a `tool_call_id` field, but the current implementation only tracks a counter rather than storing and validating the actual IDs.

**Current Behavior:**
- `ToolcallEnd` event increments a counter
- `ToolExecutionStart` shows warning if counter is 0
- No actual ID matching between events

**Proposed Fix:**
- Store the actual `tool_call_id` string from `ToolcallEnd`
- Verify that `ToolExecutionStart/End` have matching IDs
- Log errors or show warnings for ID mismatches (debugging aid)

**Files to Modify:**
- `src/cli/terminal_output.rs` - Add String field for actual ID storage

**Priority:** Low (current counter-based approach works, but full validation would be more robust)

---

### Bash Command Result Formatting Could Be More Informative

**Issue:** For bash commands, the result summary just shows the output or "completed". No exit code or command-specific context is displayed.

**Current Behavior:**
```
<bash: cargo build>
  Executing bash...
  ✓ completed  (or shows first line of output)
```

**Proposed Fix:** Extract exit code from result if available and format as:
```
  ✓ exit code: 0
  ✗ exit code: 1 - error message
```

**Files to Modify:**
- `src/cli/terminal_output.rs` - `extract_result_summary()` method

**Priority:** Low (current output is functional, just could be more informative)

---

### ToolExecutionUpdate Events Not Handled

**Issue:** ToolExecutionUpdate events are ignored, but they could stream bash command output in real-time.

**Current Behavior:** Long-running bash commands have no feedback until completion.

**Proposed Fix:** Add case for `ToolExecutionUpdate` to stream partial results for bash commands. This would show command output as it's generated, similar to how text deltas are streamed.

**Files to Modify:**
- `src/cli/terminal_output.rs` - `on_event()` method

**Priority:** Medium (improves UX for long-running commands)
**Deferred to:** Phase 4 (Interruption & Steering) or later

---

### Colors Don't Support --no-color Flag

**Issue:** Colors are hardcoded as ANSI escape codes. When Phase 5 adds `--no-color`, this will need refactoring.

**Current Behavior:**
```rust
pub fn get_tool_color(tool_name: &str) -> String {
    match tool_name {
        "read" => "\x1b[34m",  // Hardcoded ANSI
        ...
    }
}
```

**Proposed Fix:**
- Add a `no_color: bool` field to `TerminalOutputCallback`
- Add `should_color()` helper that checks both `no_color` and terminal capability
- Modify all color methods to return empty string when colors disabled

**Files to Modify:**
- `src/cli/terminal_output.rs` - Add `no_color` field and `should_color()` method
- `src/cli/commands.rs` - Add `--no-color` CLI flag
- `src/execution/engine.rs` - Propagate `no_color` flag
- `src/execution/executor.rs` - Pass `no_color` to callback constructor

**Priority:** Medium (required for Phase 5)
**Deferred to:** Phase 5 (Output Controls)
