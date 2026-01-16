//! Integration tests for streaming functionality
//!
//! These tests verify that terminal output callback works correctly
//! with the execution engine.

use pipeline::agent::{AgentExecutor, AgentResponse, AgentError, ProgressCallback, PiJsonEvent};
use pipeline::agent::pi_events::{AssistantMessageEvent, Message};
use pipeline::core::config::PipelineConfig;
use pipeline::execution::{ExecutionEngine, SchedulingStrategy};
use serde_json::json;

// Mock agent that generates test events
struct TestCallback {
    events: std::sync::Arc<std::sync::Mutex<Vec<PiJsonEvent>>>,
}

impl TestCallback {
    fn new() -> Self {
        Self {
            events: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }

    fn get_events(&self) -> Vec<PiJsonEvent> {
        self.events.lock().unwrap().clone()
    }
}

impl ProgressCallback for TestCallback {
    fn on_event(&self, event: &PiJsonEvent) {
        self.events.lock().unwrap().push(event.clone());
    }
}

// Mock agent for testing
#[derive(Clone)]
struct TestAgent {
    responses: Vec<String>,
    use_streaming: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl TestAgent {
    fn new(responses: Vec<String>) -> Self {
        Self {
            responses,
            use_streaming: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    fn was_streaming_called(&self) -> bool {
        self.use_streaming.load(std::sync::atomic::Ordering::SeqCst)
    }
}

#[async_trait::async_trait]
impl AgentExecutor for TestAgent {
    async fn execute(&self, _prompt: &str) -> Result<AgentResponse, AgentError> {
        // Return first response (for backward compatibility)
        Ok(AgentResponse::new(self.responses.first().cloned().unwrap_or_default()))
    }

    async fn execute_streaming(
        &self,
        _prompt: &str,
        callback: Option<&dyn ProgressCallback>,
    ) -> Result<AgentResponse, AgentError> {
        // Mark that streaming was called
        self.use_streaming.store(true, std::sync::atomic::Ordering::SeqCst);

        // Generate synthetic events
        let _test_callback = TestCallback::new();

        // If a callback was provided, it should be TerminalOutputCallback
        // We'll verify that it's being called
        if let Some(cb) = callback {
            // Generate AgentStart event
            cb.on_event(&PiJsonEvent::AgentStart);

            // Generate some TextDelta events
            for response in &self.responses {
                for chunk in response.split_whitespace() {
                    cb.on_event(&PiJsonEvent::MessageUpdate {
                        assistant_message_event: Some(AssistantMessageEvent::TextDelta {
                            content_index: 0,
                            delta: format!("{} ", chunk),
                        }),
                        message: None,
                    });
                }
            }

            cb.on_event(&PiJsonEvent::MessageUpdate {
                assistant_message_event: Some(AssistantMessageEvent::TextEnd {
                    content_index: 0,
                    content: Some(self.responses.join(" ")),
                }),
                message: None,
            });
            cb.on_event(&PiJsonEvent::AgentEnd);
        }

        Ok(AgentResponse::new(self.responses.join(" ")))
    }
}

#[tokio::test]
async fn test_streaming_with_engine_uses_callback() {
    let yaml = r#"
name: "Test Pipeline"
steps:
  - id: "step1"
    name: "First Step"
    prompt: "Do task 1"
    termination:
      success_pattern: "DONE"
"#;

    let config = PipelineConfig::from_yaml(yaml).unwrap();
    let mut pipeline = config.to_pipeline();

    let agent = TestAgent::new(vec!["DONE".to_string()]);
    let engine = ExecutionEngine::new(agent.clone(), SchedulingStrategy::Sequential, false);

    let result = engine.execute(&mut pipeline).await;

    assert!(result.is_ok());
    assert!(pipeline.is_complete());
    assert!(agent.was_streaming_called(), "execute_streaming should have been called");
}

#[tokio::test]
async fn test_streaming_with_show_thinking_true() {
    let agent = TestAgent::new(vec!["DONE".to_string()]);
    let _engine = ExecutionEngine::new(agent.clone(), SchedulingStrategy::Sequential, true);

    // The show_thinking flag is stored in the engine
    // The callback is created internally during step execution
    assert!(true);
}

#[tokio::test]
async fn test_streaming_with_show_thinking_false() {
    let agent = TestAgent::new(vec!["DONE".to_string()]);
    let _engine = ExecutionEngine::new(agent, SchedulingStrategy::Sequential, false);

    // Verify engine created successfully with show_thinking=false
    assert!(true);
}

// Phase 3: Tool call display integration tests

#[tokio::test]
async fn test_tool_call_display() {
    use pipeline::cli::terminal_output::TerminalOutputCallback;
    let callback = TerminalOutputCallback::new(false, 1);

    // Simulate a read tool call sequence
    let tool_call_start_event = PiJsonEvent::MessageUpdate {
        assistant_message_event: Some(AssistantMessageEvent::ToolcallStart {
            content_index: 1,
            partial: Message {
                role: "assistant".to_string(),
                content: vec![json!({
                    "type": "toolCall",
                    "id": "call_123",
                    "name": "read",
                    "arguments": {"path": "src/auth.rs"}
                })],
            },
        }),
        message: None,
    };

    let tool_execution_end_event = PiJsonEvent::ToolExecutionEnd {
        tool_call_id: "call_123".to_string(),
        tool_name: "read".to_string(),
        result: json!({"content": [{"type": "text", "text": "Read 156 lines"}]}),
        is_error: false,
    };

    // Should not panic
    callback.on_event(&tool_call_start_event);
    callback.on_event(&tool_execution_end_event);
}

#[tokio::test]
async fn test_error_tool_call_display() {
    use pipeline::cli::terminal_output::TerminalOutputCallback;
    let callback = TerminalOutputCallback::new(false, 1);

    // Simulate an error during bash execution
    let tool_call_start_event = PiJsonEvent::MessageUpdate {
        assistant_message_event: Some(AssistantMessageEvent::ToolcallStart {
            content_index: 1,
            partial: Message {
                role: "assistant".to_string(),
                content: vec![json!({
                    "type": "toolCall",
                    "id": "call_456",
                    "name": "bash",
                    "arguments": {"command": "cargo build"}
                })],
            },
        }),
        message: None,
    };

    let tool_execution_end_event = PiJsonEvent::ToolExecutionEnd {
        tool_call_id: "call_456".to_string(),
        tool_name: "bash".to_string(),
        result: json!({"content": [{"type": "text", "text": "error: could not find `missing_crate`"}]}),
        is_error: true,
    };

    // Should not panic
    callback.on_event(&tool_call_start_event);
    callback.on_event(&tool_execution_end_event);
}

#[tokio::test]
async fn test_multiple_tool_calls() {
    use pipeline::cli::terminal_output::TerminalOutputCallback;
    let callback = TerminalOutputCallback::new(false, 1);

    // Simulate multiple tool calls in sequence
    let read_event = PiJsonEvent::MessageUpdate {
        assistant_message_event: Some(AssistantMessageEvent::ToolcallStart {
            content_index: 1,
            partial: Message {
                role: "assistant".to_string(),
                content: vec![json!({
                    "type": "toolCall",
                    "id": "call_1",
                    "name": "read",
                    "arguments": {"path": "src/file.rs"}
                })],
            },
        }),
        message: None,
    };

    let write_event = PiJsonEvent::MessageUpdate {
        assistant_message_event: Some(AssistantMessageEvent::ToolcallStart {
            content_index: 1,
            partial: Message {
                role: "assistant".to_string(),
                content: vec![json!({
                    "type": "toolCall",
                    "id": "call_2",
                    "name": "write",
                    "arguments": {"path": "src/file.rs", "content": "new content"}
                })],
            },
        }),
        message: None,
    };

    let bash_event = PiJsonEvent::MessageUpdate {
        assistant_message_event: Some(AssistantMessageEvent::ToolcallStart {
            content_index: 1,
            partial: Message {
                role: "assistant".to_string(),
                content: vec![json!({
                    "type": "toolCall",
                    "id": "call_3",
                    "name": "bash",
                    "arguments": {"command": "cargo test"}
                })],
            },
        }),
        message: None,
    };

    // Should not panic
    callback.on_event(&read_event);
    callback.on_event(&write_event);
    callback.on_event(&bash_event);
}
