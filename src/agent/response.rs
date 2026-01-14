//! Agent response types

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Error types for agent operations
#[derive(Debug, Error)]
pub enum AgentError {
    #[error("API error: {0}")]
    Api(String),

    #[error("Timeout after {0} seconds")]
    Timeout(u64),

    #[error("Internal error: {0}")]
    Internal(String),
}

/// Response from the agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResponse {
    /// The response content
    pub content: String,

    /// Whether the response is complete (for streaming)
    pub done: bool,

    /// Token usage information (if available)
    pub usage: Option<TokenUsage>,
}

impl AgentResponse {
    /// Create a new agent response
    #[allow(dead_code)]
    pub fn new(content: String) -> Self {
        Self {
            content,
            done: true,
            usage: None,
        }
    }
}

/// Token usage information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_response_creation() {
        let response = AgentResponse {
            content: "Hello, world!".to_string(),
            done: true,
            usage: None,
        };
        assert_eq!(response.content, "Hello, world!");
        assert!(response.done);
        assert!(response.usage.is_none());
    }
}
