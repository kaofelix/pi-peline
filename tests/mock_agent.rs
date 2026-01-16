//! Mock agent for deterministic, fast unit tests

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use async_trait::async_trait;
use pipeline::{AgentExecutor, AgentResponse, AgentError, PiJsonEvent, ProgressCallback};

/// Mock agent that returns predefined responses
///
/// This is useful for:
/// - Fast, deterministic tests without subprocess overhead
/// - Testing step chaining (plan → implement → review)
/// - Testing continuation/retry behavior
/// - Testing branching (on_success, on_failure)
/// - Testing retry limits
pub struct MockAgent {
    responses: Arc<Vec<String>>,
    index: Arc<AtomicUsize>,
    simulate_delay: Option<std::time::Duration>,
}

impl MockAgent {
    /// Create a new mock agent with predefined responses
    pub fn new(responses: Vec<String>) -> Self {
        Self {
            responses: Arc::new(responses),
            index: Arc::new(AtomicUsize::new(0)),
            simulate_delay: None,
        }
    }

    /// Add artificial delay to simulate slow agent
    pub fn with_delay(mut self, delay: std::time::Duration) -> Self {
        self.simulate_delay = Some(delay);
        self
    }

    /// Get number of responses remaining
    pub fn remaining(&self) -> usize {
        self.responses.len() - self.index.load(Ordering::SeqCst)
    }

    /// Reset the response index to start from the beginning
    pub fn reset(&self) {
        self.index.store(0, Ordering::SeqCst);
    }

    /// Get the current response index (how many have been used)
    pub fn current_index(&self) -> usize {
        self.index.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl AgentExecutor for MockAgent {
    async fn execute(&self, prompt: &str) -> Result<AgentResponse, AgentError> {
        // Simulate delay if configured
        if let Some(delay) = self.simulate_delay {
            tokio::time::sleep(delay).await;
        }

        let idx = self.index.fetch_add(1, Ordering::SeqCst);

        if idx >= self.responses.len() {
            return Err(AgentError::Internal(format!(
                "MockAgent: No response available for request {} (have {} responses). Prompt: {}",
                idx + 1,
                self.responses.len(),
                prompt
            )));
        }

        tracing::debug!(
            "[MockAgent] Responding to request {}: {} bytes, prompt prefix: {}",
            idx,
            self.responses[idx].len(),
            &prompt[..prompt.len().min(50)]
        );

        Ok(AgentResponse::new(self.responses[idx].clone()))
    }

    async fn execute_streaming(
        &self,
        prompt: &str,
        callback: Option<&dyn ProgressCallback>,
    ) -> Result<AgentResponse, AgentError> {
        // Get the same response as execute() would
        let idx = self.index.fetch_add(1, Ordering::SeqCst);

        if idx >= self.responses.len() {
            return Err(AgentError::Internal(format!(
                "MockAgent: No response available for request {} (have {} responses). Prompt: {}",
                idx + 1,
                self.responses.len(),
                prompt
            )));
        }

        let response = &self.responses[idx];

        // Simulate delay if configured
        if let Some(delay) = self.simulate_delay {
            tokio::time::sleep(delay).await;
        }

        // Generate synthetic events if callback provided
        if let Some(cb) = callback {
            // AgentStart
            cb.on_event(&PiJsonEvent::AgentStart);

            // TextDelta events (split by characters for simplicity)
            for ch in response.chars() {
                cb.on_event(&PiJsonEvent::TextDelta {
                    delta: ch.to_string(),
                });
            }

            // TextEnd
            cb.on_event(&PiJsonEvent::TextEnd {
                content: Some(response.clone()),
            });

            // AgentEnd
            cb.on_event(&PiJsonEvent::AgentEnd);
        }

        tracing::debug!(
            "[MockAgent] Streaming response to request {}: {} bytes, prompt prefix: {}",
            idx,
            response.len(),
            &prompt[..prompt.len().min(50)]
        );

        Ok(AgentResponse::new(response.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_agent_returns_responses() {
        let responses = vec![
            "First response".to_string(),
            "Second response".to_string(),
            "Third response ✅ DONE".to_string(),
        ];
        let agent = MockAgent::new(responses);

        let r1 = agent.execute("").await.unwrap();
        assert!(r1.content.contains("First"));

        let r2 = agent.execute("").await.unwrap();
        assert!(r2.content.contains("Second"));

        let r3 = agent.execute("").await.unwrap();
        assert!(r3.content.contains("Third"));
    }

    #[tokio::test]
    async fn test_mock_agent_exhausted() {
        let agent = MockAgent::new(vec!["Only one".to_string()]);
        agent.execute("").await.unwrap();

        let result = agent.execute("").await;
        assert!(result.is_err());

        if let Err(AgentError::Internal(msg)) = result {
            assert!(msg.contains("No response available"));
        } else {
            panic!("Expected AgentError::Internal");
        }
    }

    #[tokio::test]
    async fn test_mock_agent_remaining() {
        let agent = MockAgent::new(vec!["A".to_string(), "B".to_string(), "C".to_string()]);
        assert_eq!(agent.remaining(), 3);

        agent.execute("").await.unwrap();
        assert_eq!(agent.remaining(), 2);

        agent.execute("").await.unwrap();
        assert_eq!(agent.remaining(), 1);

        agent.execute("").await.unwrap();
        assert_eq!(agent.remaining(), 0);
    }

    #[tokio::test]
    async fn test_mock_agent_reset() {
        let agent = MockAgent::new(vec!["First".to_string(), "Second".to_string()]);

        let r1 = agent.execute("").await.unwrap();
        assert!(r1.content.contains("First"));

        agent.reset();

        let r2 = agent.execute("").await.unwrap();
        assert!(r2.content.contains("First")); // Should be "First" again
    }

    #[tokio::test]
    async fn test_mock_agent_with_delay() {
        let agent = MockAgent::new(vec!["Delayed".to_string()])
            .with_delay(std::time::Duration::from_millis(100));

        let start = std::time::Instant::now();
        let result = agent.execute("").await.unwrap();
        let elapsed = start.elapsed();

        assert!(result.content.contains("Delayed"));
        assert!(elapsed >= std::time::Duration::from_millis(90)); // Allow some margin
        assert!(elapsed < std::time::Duration::from_millis(200));
    }

    #[tokio::test]
    async fn test_mock_agent_current_index() {
        let agent = MockAgent::new(vec!["A".to_string(), "B".to_string(), "C".to_string()]);

        assert_eq!(agent.current_index(), 0);

        agent.execute("").await.unwrap();
        assert_eq!(agent.current_index(), 1);

        agent.execute("").await.unwrap();
        assert_eq!(agent.current_index(), 2);

        agent.execute("").await.unwrap();
        assert_eq!(agent.current_index(), 3);
    }

    // Tests for execute_streaming
    #[tokio::test]
    async fn test_mock_agent_streaming_with_no_callback() {
        let agent = MockAgent::new(vec!["Hello World".to_string()]);
        let result = agent.execute_streaming("", None).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.content, "Hello World");
        assert!(response.done);
    }

    #[tokio::test]
    async fn test_mock_agent_streaming_with_callback() {
        use std::sync::{Arc, Mutex};

        struct TestCallback {
            events: Arc<Mutex<Vec<PiJsonEvent>>>,
        }

        impl TestCallback {
            fn new() -> Self {
                Self {
                    events: Arc::new(Mutex::new(Vec::new())),
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

        let agent = MockAgent::new(vec!["Hi".to_string()]);
        let callback = TestCallback::new();

        let result = agent.execute_streaming("", Some(&callback)).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.content, "Hi");

        // Check events
        let events = callback.get_events();
        assert_eq!(events.len(), 5); // AgentStart, 2x TextDelta, TextEnd, AgentEnd
        assert_eq!(events[0], PiJsonEvent::AgentStart);
        assert_eq!(events[1], PiJsonEvent::TextDelta { delta: "H".to_string() });
        assert_eq!(events[2], PiJsonEvent::TextDelta { delta: "i".to_string() });
        assert_eq!(events[3], PiJsonEvent::TextEnd { content: Some("Hi".to_string()) });
        assert_eq!(events[4], PiJsonEvent::AgentEnd);
    }

    #[tokio::test]
    async fn test_mock_agent_streaming_multiple_calls() {
        use std::sync::{Arc, Mutex};

        struct CountingCallback {
            count: Arc<Mutex<usize>>,
        }

        impl ProgressCallback for CountingCallback {
            fn on_event(&self, _event: &PiJsonEvent) {
                *self.count.lock().unwrap() += 1;
            }
        }

        let agent = MockAgent::new(vec![
            "First".to_string(),
            "Second".to_string(),
        ]);

        let count = Arc::new(Mutex::new(0));

        let cb1 = CountingCallback { count: count.clone() };
        agent.execute_streaming("", Some(&cb1)).await.unwrap();

        let cb2 = CountingCallback { count: count.clone() };
        agent.execute_streaming("", Some(&cb2)).await.unwrap();

        // Events per call: AgentStart + N*TextDelta + TextEnd + AgentEnd
        // "First" (5 chars): AgentStart + 5*TextDelta + TextEnd + AgentEnd = 8 events
        // "Second" (6 chars): AgentStart + 6*TextDelta + TextEnd + AgentEnd = 9 events
        assert_eq!(*count.lock().unwrap(), 17);
    }
}
