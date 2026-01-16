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

## Broken Tests

### Smoke Tests Outdated

**Issue:** Smoke tests in `tests/smoke_test.rs` fail with "No termination pattern found"

**Root Cause:** The tests use `PiAgentClient.execute()` which calls the non-streaming `subprocess_client.execute()` (text mode), not the streaming JSON mode. The termination patterns aren't found because the output is in text mode, not parsed JSON.

**Status:** Tests need to be updated to use streaming mode or use MockAgent instead.

**Priority:** Medium (smoke tests useful for catching regressions)

**Proposed Fix:**
1. Update tests to use MockAgent (which doesn't require Pi CLI)
2. Or create a streaming-aware test runner
3. For now, rely on lib tests for smoke testing

**Date Identified:** 2026-01-16

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
