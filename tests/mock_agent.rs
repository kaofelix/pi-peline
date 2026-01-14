//! Mock agent for deterministic, fast unit tests

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use async_trait::async_trait;
use pipeline::{AgentExecutor, AgentResponse, AgentError};

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
}
