//! Agent response types

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Error types for agent operations
#[derive(Debug, Error)]
pub enum AgentError {
    #[error("Network error: {0}")]
    Network(String),

    #[error("API error: {0}")]
    Api(String),

    #[error("Timeout after {0} seconds")]
    Timeout(u64),

    #[error("Authentication failed")]
    Authentication,

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Rate limited: retry after {0} seconds")]
    RateLimited(u64),

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

/// Token usage information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

impl AgentResponse {
    pub fn new(content: String) -> Self {
        Self {
            content,
            done: true,
            usage: None,
        }
    }

    pub fn with_usage(mut self, usage: TokenUsage) -> Self {
        self.usage = Some(usage);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_response() {
        let response = AgentResponse::new("Hello, world!".to_string());
        assert_eq!(response.content, "Hello, world!");
        assert!(response.done);
        assert!(response.usage.is_none());
    }

    #[test]
    fn test_agent_response_with_usage() {
        let usage = TokenUsage {
            prompt_tokens: 10,
            completion_tokens: 20,
            total_tokens: 30,
        };
        let response = AgentResponse::new("Hello".to_string()).with_usage(usage);

        assert_eq!(response.usage.unwrap().total_tokens, 30);
    }
}
