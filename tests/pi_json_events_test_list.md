# Pi JSON Event Parsing - TDD Test List

## Progress
- ✅ = Test passes
- ❌ = Test fails
- ⏸️ = Not yet written

---

## Category 1: Simple Events (no nested data)
- ✅ `agent_start` - Simple no-payload event (existing test)
- ✅ `agent_end` - Simple no-payload event (existing test)
- ✅ `turn_start` - Simple no-payload event
- ✅ `turn_end` - Has message and toolResults arrays
- ⚠️ `message_start` - Has message object (structure exists, no dedicated test)
- ⚠️ `message_end` - Has message object (structure exists, no dedicated test)

## Category 2: Session Header
- ✅ `session` - Has version, id, timestamp, cwd (existing test)

## Category 3: MessageUpdate with Nested AssistantMessageEvent
- ✅ `thinking_start` - Has contentIndex, partial message
- ✅ `thinking_delta` - Has contentIndex, delta string, partial
- ✅ `thinking_end` - Has contentIndex, content string, partial
- ✅ `text_start` - Has contentIndex, partial
- ✅ `text_delta` - Has contentIndex, delta string, partial (existing test)
- ✅ `text_end` - Has contentIndex, content string, partial (existing test)
- ✅ `toolcall_start` - Has contentIndex, partial
- ✅ `toolcall_delta` - Has contentIndex, delta string, partial
- ✅ `toolcall_end` - Has contentIndex, toolCall object, partial

## Category 4: Tool Execution Events
- ✅ `tool_execution_start` - Has toolCallId, toolName, args
- ✅ `tool_execution_update` - Has toolCallId, toolName, args, partialResult
- ✅ `tool_execution_end` - Has toolCallId, toolName, result, isError

## Category 5: Message Object Content Types
- [ ] Message with `text` content
- [ ] Message with `thinking` content
- [ ] Message with `toolCall` content
- [ ] Message with mixed content (text + thinking + toolCall)

## Category 6: Error Handling
- [ ] Unknown event type falls back to Unknown variant
- [ ] Missing required field causes parse failure
- [ ] Invalid field type causes parse failure

## Category 7: Edge Cases
- [ ] Empty message content array
- [ ] Tool result with image content
- [ ] Message with empty usage/cost fields

---

## Notes
- All JSON examples sourced from actual `pi --mode json` output
- Some tests already exist in pi_events.rs (agent_start, agent_end, session, etc.)
- Focus on missing variants first: thinking_start, text_start, toolcall_start, etc.
- **Test Status**: 20 tests passing ✅
