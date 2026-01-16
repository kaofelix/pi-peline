# Observability Implementation Plan

---

## Goal

Enable real-time observation of pipeline execution so users can see the agent's thoughts, tool calls, and output as they happen. This allows for steering - stopping execution when things go wrong direction and correcting course.

---

## Why This Matters

**Current Problem:** Running `pi-peline` feels like a black box. You start it, wait minutes, then see the result. If the agent goes off-track, you only discover after it's too late - wasting time and API credits.

**Desired Outcome:** Watch the agent work like you would in interactive mode. See it thinking, reading files, writing code, running commands. Catch mistakes early. Stop and redirect when needed.

---

## Validation Results (Complete)

We've validated Pi's `--mode json` output and confirmed:

### Top-Level Event Types
- `agent_start` / `agent_end` - Execution lifecycle
- `turn_start` / `turn_end` - Conversation turns
- `message_start` / `message_end` / `message_update` - Message lifecycle

### Assistant Message Events
- `thinking_delta` / `thinking_end` - Reasoning (hidden by default)
- `text_delta` / `text_end` - Actual output (shown in real-time)
- `toolcall_start` / `toolcall_delta` / `toolcall_end` - Tool invocations

### Tool Execution Events
- `tool_execution_start` - Tool begins running
- `tool_execution_update` - Tool progress (streaming output)
- `tool_execution_end` - Tool completion (includes `isError` for failure detection)

### Key Findings
1. Tool calls are embedded in `message.content` as `type: "toolCall"` objects
2. One JSON object per line - simple to parse
3. Text deltas stream token-by-token - can print directly
4. Thinking deltas are very verbose - hide by default
5. Tool errors are structured via `isError` boolean

**Conclusion:** The approach is sound. Ready to implement.

---

## Desired Outcomes

1. **Real-time Visibility** - See every token, tool call, and operation as it happens
2. **Context** - Understand *what* agent is doing (reading, writing, running commands)
3. **Steering** - Stop execution mid-step and decide: retry, continue, or reroute
4. **Confidence** - Trust that pipelines are proceeding as intended
5. **Debugging** - Enable deeper inspection when things go wrong

---

## High-Level Approach

**Core Strategy:** Switch from `--mode text --no-session` (blocking) to `--mode json --print` (streaming) and process events line-by-line.

**Why This Works:**
- JSON mode gives structured, parseable events
- Streaming is real-time (no waiting for completion)
- Same events can power terminal output AND future web UI
- Foundation for interruption and steering

---

## Implementation Phases

### Phase 1: Core Streaming Infrastructure

**Objective:** Replace blocking command execution with streaming JSON parser.

**Work:**
1. Update `PiAgentClient` to spawn subprocess with `stdout` piped
2. Read stdout line-by-line, parsing each line as JSON
3. Define `PiJsonEvent` enum to represent all event types
4. Accumulate text deltas into final response
5. Maintain existing `AgentResponse` return type

**Outcome:**
- Agent runs with JSON streaming
- All tests pass
- Foundation for event processing established

**Risk:** Low - purely internal refactoring

---

### Phase 2: Live Output Display

**Objective:** Stream agent output to terminal in real-time.

**Work:**
1. Print section headers for each step (`[1/3] Planning`)
2. Print `text_delta` content immediately as it arrives
3. Use horizontal rules (`─────`) between steps for separation
4. Optionally print `thinking_delta` behind `--show-thinking` flag
5. Ensure stdout is flushed after each delta for immediate display

**Outcome:**
- User watches agent work like in interactive mode
- Immediate feedback on what's happening
- No more "black box" waiting

**Risk:** Low - display logic only

---

### Phase 3: Tool Call Formatting

**Objective:** Make tool calls visually distinct and actionable.

**Work:**
1. Detect tool calls in the `message.content` array
2. Format tool invocation indicators:
   - `<read: path/to/file>` in blue
   - `<write: path/to/file>` in green
   - `<bash: command>` in yellow
   - `<edit: path/to/file>` in cyan
3. Show tool arguments (file paths, commands) for context
4. Display tool results (`tool_execution_end`) with status:
   - Success checkmarks for completed tools
   - Error indicators (`isError: true`) in red

**Outcome:**
- Easy to scan for file operations and commands
- Clear visual feedback on tool execution
- Errors stand out immediately

**Risk:** Medium - requires parsing embedded tool call structure

---

### Phase 4: Interruption & Steering

**Objective:** Allow user to pause execution and make decisions.

**Work:**
1. Set up Ctrl+C signal listener during streaming
2. On interrupt:
   - Stop reading new JSON events
   - Print separator showing interruption point
   - Display last 10-20 lines of accumulated output as context
   - Present steering menu
3. Handle user choices:
   - **[R]etry**: Rerun current step with modified prompt
   - **[C]ontinue**: Resume streaming from where it left off
   - **[R]oute**: Jump to a different step ID
   - **[A]bort**: Stop entire pipeline
4. Terminate the `pi` subprocess cleanly

**Outcome:**
- Agent goes off-track → immediate stop → correction
- No need to wait for step completion to fix
- Save time on expensive long-running steps

**Risk:** Medium - requires subprocess management and state handling

---

### Phase 5: Output Controls

**Objective:** Let users tune verbosity for different use cases.

**Work:**
1. Add `--show-thinking` flag to display reasoning (off by default)
2. Add `--quiet` flag to suppress output (show only step completion status)
3. Add `--output-level` option: `minimal`, `normal`, `verbose`
4. Add `--no-color` flag for CI/logging scenarios
5. Consider `--filter-tools` to hide specific tool types

**Outcome:**
- Normal use: clean, focused output
- Debugging: full visibility with thinking
- Automation: minimal noise for logs
- Customizable based on context

**Risk:** Low - conditional display logic

---

### Phase 6: Error Handling & Recovery

**Objective:** Gracefully handle and display errors.

**Work:**
1. Detect tool failures via `isError` boolean in `tool_execution_end`
2. Display tool error output clearly (red, with context)
3. Continue execution if possible (don't always abort on errors)
4. Add error context to accumulated response
5. Support retry logic for transient failures

**Outcome:**
- Errors are visible and actionable
- Pipeline can continue through recoverable errors
- Better debugging information

**Risk:** Low-Medium - need to balance strict vs permissive error handling

---

## Data Flow

```
┌─────────────────────────────────────────────────────────────────┐
│                      pi-peline CLI                              │
│  ┌────────────────────────────────────────────────────────────┐  │
│  │                  ExecutionEngine                           │  │
│  │  - Manages step lifecycle                                 │  │
│  │  - Emits pipeline-level events                             │  │
│  └──────────────┬─────────────────────────────────────────────┘  │
│                 │                                               │
│                 ▼                                               │
│  ┌────────────────────────────────────────────────────────────┐  │
│  │                 StepExecutor                              │  │
│  │  - Builds effective prompt                                 │  │
│  │  - Delegates to PiAgentClient                            │  │
│  │  - Receives final AgentResponse                           │  │
│  └──────────────┬─────────────────────────────────────────────┘  │
│                 │                                               │
│                 ▼                                               │
│  ┌────────────────────────────────────────────────────────────┐  │
│  │                 PiAgentClient (NEW)                        │  │
│  │  Spawns: pi --mode json --print <prompt>               │  │
│  │  Reads stdout line-by-line                               │  │
│  │  Parses: {"type":"text_delta","delta":"..."}           │  │
│  │  - For each event:                                      │  │
│  │    • Call progress callback (display to terminal)          │  │
│  │    • Accumulate text deltas into buffer                   │  │
│  │  - On completion: return AgentResponse { content, usage }  │  │
│  └──────────────┬─────────────────────────────────────────────┘  │
│                 │                                               │
│                 ▼                                               │
│  ┌────────────────────────────────────────────────────────────┐  │
│  │                  Progress Callback                          │  │
│  │  - Text delta: print directly to stdout                  │  │
│  │  - Tool call: format and display                        │  │
│  │  - Tool execution: show results/errors                  │  │
│  │  - Listen for Ctrl+C signal                            │  │
│  └────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Pi CLI (subprocess)                         │
│  - Streams JSON events line-by-line                           │
│  - Includes: text_delta, toolcall_*, tool_execution_*      │
└─────────────────────────────────────────────────────────────────┘
```

---

## Terminal UX Evolution

### Before (Current)
```
$ pi-peline run pipeline.yaml

🔄 Running pipeline...
✅ Pipeline completed successfully
```
*User waits 5 minutes with zero feedback*

---

### After Phase 2 (Real-time Output)
```
$ pi-peline run pipeline.yaml

🔄 Running pipeline: Feature Development Pipeline

─────────────────────────────────────────────────────────────
[1/3] Planning
─────────────────────────────────────────────────────────────

I'll analyze the requirements and create a detailed plan.

Let me start by reading the README to understand the project structure...

<read: README.md>
✓ Read 234 lines

Based on the project, here's my implementation plan:
1. Add authentication module
2. Implement user CRUD operations
3. Add JWT token handling
4. Create protected route middleware
5. Add unit tests

I'll now implement each component...

<write: src/auth/auth.rs>
✓ Created auth.rs with 156 lines

<write: src/auth/middleware.rs>
✓ Created middleware.rs with 89 lines

✅ PLAN_COMPLETE
─────────────────────────────────────────────────────────────
```

---

### After Phase 3 (Colored Tool Calls)
```
─────────────────────────────────────────────────────────────
[2/3] Implementation
─────────────────────────────────────────────────────────────

I'll now implement the authentication system...

<read: src/auth/auth.rs>
✓ Read 156 lines

<edit: src/auth/auth.rs>
+ pub struct User { pub id: String, ... }
✓ Modified 12 lines

<read: package.json>
✓ Read 42 lines

I need to add bcrypt for password hashing...

<bash: npm install bcryptjs>
✓ bcryptjs@2.4.3 installed

Continuing with database integration...

<read: src/database/connection.rs>
✓ Read 78 lines

<write: src/database/schema.sql>
✓ Created schema.sql with 234 lines
```

---

### After Phase 4 (Steering)
```
[2/3] Implementation
─────────────────────────────────────────────────────────────

I'll now implement the authentication system...

<read: src/auth/auth.rs>
✓ Read 156 lines

<write: src/auth/middleware.rs>
✓ Created middleware.rs

I'll use local storage for simplicity instead of PostgreSQL...

[User presses Ctrl+C]

─────────────────────────────────────────────────────────────
⚠️  Execution interrupted
─────────────────────────────────────────────────────────────

Last output:
  I'll use local storage for simplicity instead of PostgreSQL...

─────────────────────────────────────────────────────────────
What would you like to do?

  [R]etry this step with modified prompt
  [C]ontinue (resume streaming)
  [R]oute to step: [step-id]
  [A]bort pipeline

Your choice: R

Enter new prompt or modification:
  Use PostgreSQL for the database, not local storage
─────────────────────────────────────────────────────────────
```

---

## File Structure Changes

```
src/
├── agent/
│   ├── pi_client.rs         # MODIFY: Add JSON streaming
│   ├── pi_events.rs         # NEW: Event type definitions
│   └── mod.rs
├── execution/
│   ├── executor.rs         # MODIFY: Handle streaming events
│   └── mod.rs
├── cli/
│   ├── output.rs            # MODIFY: Live display formatting
│   ├── steering.rs          # NEW: Interruption logic
│   └── commands.rs
└── main.rs                 # MODIFY: Add flags, signal handling
```

---

## Key Design Decisions

### 1. Print Directly vs Buffer & Flush

**Decision:** Print each delta immediately, flush stdout

**Why:**
- Maximum real-time visibility
- No artificial delay
- Simple implementation
- Modern terminals handle frequent updates well

---

### 2. Show Thinking by Default or Behind Flag

**Decision:** Behind `--show-thinking` flag (off by default)

**Why:**
- Thinking deltas are extremely verbose (every token of reasoning)
- Overwhelming for normal usage
- Focus on actual output (what agent *does*, not just what it *thinks*)
- Enable when debugging direction or understanding agent behavior

---

### 3. How to Detect Tool Calls

**Decision:** Parse from `message.content` array in `message_update` events

**Why:**
- Tool calls are embedded as `type: "toolCall"` objects in content
- Separate `toolcall_start`/`delta`/`end` events provide progress
- Structured, reliable detection
- Can extract tool name, arguments, and ID

---

### 4. How Much Context to Show on Interrupt

**Decision:** Last 10-20 lines of accumulated output

**Why:**
- Enough context to remember what was happening
- Not overwhelming
- Focused on decision point (where it went wrong)

---

### 5. Resume vs Retry on Steering

**Decision:** Support both

**Retry**: Rerun entire step from scratch with modified prompt
**Continue**: Resume from exact interruption point

**Why:**
- Retry is safer (fresh start with new direction)
- Continue is useful for false alarms
- Gives user control based on situation

---

## Success Metrics

| Metric | How to Measure | Target |
|--------|----------------|--------|
| Real-time visibility | Lag between agent output and terminal display | < 100ms |
| Readability | User can follow agent's actions without confusion | Qualitative - user testing |
| Steering responsiveness | Time from Ctrl+C to choice prompt | < 100ms |
| Tool call detection | % of tool calls correctly identified and formatted | > 95% |
| Error clarity | User can identify what went wrong and why | Qualitative - user testing |

---

## Risks & Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Output too noisy/hard to read | High | Add separators, formatting, output controls |
| Terminal flickering with fast output | Medium | Minimal buffering, optimized rendering |
| Tool call parsing breaks on Pi update | High | Pin to compatible version, add graceful fallback |
| Ctrl+C doesn't kill subprocess cleanly | Medium | Use tokio signal handling, test thoroughly |
| `isError` detection misses some errors | Low-Medium | Also check exit codes, output content |
| Memory leak with very long runs | Low | Periodically flush accumulated text to disk |

---

## Estimated Timeline

| Phase | Complexity | Time |
|-------|------------|------|
| Phase 1: Core Streaming | Medium | 2-3 hours |
| Phase 2: Live Output Display | Low | 1-2 hours |
| Phase 3: Tool Call Formatting | Medium | 2-3 hours |
| Phase 4: Interruption & Steering | Medium | 2-3 hours |
| Phase 5: Output Controls | Low | 1 hour |
| Phase 6: Error Handling | Low-Medium | 1-2 hours |
| Testing & Polish | - | 2-3 hours |
| **Total** | | **11-17 hours** |

---

## What This Enables

### Immediate Value (After Phases 1-2)
- ✅ Watch agent work in real-time
- ✅ See every file read, write, and edit
- ✅ Observe command execution
- ✅ Build intuition for prompt effectiveness

### Extended Value (After Phase 4)
- ✅ Stop agent going off-track immediately
- ✅ Correct course without restarting entire pipeline
- ✅ Learn from what works vs doesn't work
- ✅ Save significant time on long pipelines

### Future Foundation
- ✅ Same events can power web UI via SSE/WebSocket
- ✅ Session persistence for replay
- ✅ Checkpoint system for human-in-the-loop workflows
- ✅ Distributed execution monitoring

---

## Next Steps

1. **Start Phase 1:** Implement `PiJsonEvent` enum and line-by-line parser
2. **Test with existing pipeline:** Verify streaming works without breaking functionality
3. **Iterate:** Move through phases 2-6, testing at each stage
4. **Dogfood:** Use the new observability to run and refine actual pipelines
5. **Gather feedback:** Adjust based on real-world usage

---

## Related Work

Once observability is in place, these features become easier:

- **Human-in-the-loop checkpoints** - Pause at specific steps for approval
- **Shell script quality gates** - Show command output in real-time
- **Web UI** - Stream same events to browser
- **Session replay** - Save and review interesting runs
- **Pipeline comparison** - See differences between runs

---

## Appendix: Event Type Reference

### Top-Level Events
- `agent_start` / `agent_end` - Pipeline execution boundaries
- `turn_start` / `turn_end` - Single prompt-response turn
- `message_start` / `message_end` / `message_update` - Message lifecycle

### Content Events
- `thinking_delta` - Reasoning tokens (hidden by default)
- `thinking_end` - Final reasoning content
- `text_delta` - Actual output tokens (shown in real-time)
- `text_end` - Final text content
- `toolcall_start` - Tool invocation begins
- `toolcall_delta` - Tool argument streaming
- `toolcall_end` - Tool invocation complete

### Execution Events
- `tool_execution_start` - Tool begins running
- `tool_execution_update` - Tool output streaming (e.g., bash output)
- `tool_execution_end` - Tool completion (includes `isError`, `output`, `exitCode`)
