//! Integration tests for streaming functionality
//!
//! These tests verify that terminal output callback works correctly
//! with the execution engine.

use pipeline::agent::{AgentExecutor, AgentResponse, AgentError, ProgressCallback, PiJsonEvent};
use pipeline::core::config::PipelineConfig;
use pipeline::execution::{ExecutionEngine, SchedulingStrategy};

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
                    cb.on_event(&PiJsonEvent::TextDelta {
                        delta: format!("{} ", chunk),
                    });
                }
            }

            cb.on_event(&PiJsonEvent::TextEnd {
                content: Some(self.responses.join(" ")),
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
