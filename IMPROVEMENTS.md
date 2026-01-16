# Improvements

Track technical debt, bugs, and future improvements for the pi-peline project.

---

## Bugs Fixed

### Session Files Not Being Created (Phase 2 Regression)

**Issue:** Phase 2 brought back the streaming code with `--no-session` flag, preventing session files from being created.

**Root Cause:** The streaming method in `subprocess_client.rs` still had `--no-session` from the original implementation.

**Fix:** Removed `--no-session` from the streaming args in `src/agent/subprocess_client.rs`.

**Date Fixed:** 2026-01-16

---

### JSON Parsing Failures for Tool Execution Events

**Issue:** Warnings when parsing tool execution events due to field name mismatch:
```
WARN pipeline::agent::subprocess_client: Failed to parse JSON line: missing field `tool_call_id`
```

**Root Cause:** Pi outputs field names in camelCase (`toolCallId`, `isError`, `exitCode`) but the schema expected snake_case. Also, tool execution events have extra fields (`toolName`, `args`, `result`, `partialResult`) that weren't captured.

**Fix:**
1. Added `#[serde(alias = "toolCallId")]`, `#[serde(alias = "isError")]` to accept both camelCase and snake_case
2. Added optional fields for tool execution events: `tool_name`, `args`, `result`, `partial_result`
3. Updated tests to use camelCase field names

**Files Modified:** `src/agent/pi_events.rs`

**Date Fixed:** 2026-01-16

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
