//! Pi agent client for executing prompts

pub mod client;
pub mod response;

use async_trait::async_trait;
pub use client::{AgentClientConfig};
pub use response::{AgentResponse, AgentError};

/// Trait for agent execution - allows for different implementations
#[async_trait]
pub trait AgentExecutor: Send + Sync {
    /// Execute a prompt and stream the response
    async fn execute(&self, prompt: &str) -> Result<AgentResponse, AgentError>;
}

/// Default Pi agent client (placeholder - implement based on actual Pi API)
#[derive(Debug, Clone)]
pub struct PiAgentClient {
    config: AgentClientConfig,
}

impl PiAgentClient {
    pub fn new(config: AgentClientConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl AgentExecutor for PiAgentClient {
    async fn execute(&self, prompt: &str) -> Result<AgentResponse, AgentError> {
        // TODO: Implement actual Pi agent API call
        // For now, this is a placeholder that returns a mock response
        Ok(AgentResponse {
            content: format!("Response to: {}", prompt),
            done: true,
            usage: None,
        })
    }
}
