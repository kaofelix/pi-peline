//! Pi agent client for executing prompts

pub mod client;
pub mod response;
pub mod subprocess_client;

use async_trait::async_trait;
pub use client::{AgentClientConfig};
pub use response::{AgentResponse, AgentError};
pub use subprocess_client::PiSubprocessClient;

/// Trait for agent execution - allows for different implementations
#[async_trait]
pub trait AgentExecutor: Send + Sync {
    /// Execute a prompt and stream the response
    async fn execute(&self, prompt: &str) -> Result<AgentResponse, AgentError>;
}

/// Pi agent client that calls pi CLI as a subprocess
#[derive(Debug, Clone)]
pub struct PiAgentClient {
    /// The subprocess client that handles the actual pi execution
    subprocess_client: PiSubprocessClient,
}

impl PiAgentClient {
    /// Create a new Pi agent client
    ///
    /// # Arguments
    /// * `config` - Configuration for the agent client
    ///
    /// The `config.endpoint` field is used as the path to the pi executable.
    /// If not provided, defaults to "pi" (assuming it's on PATH).
    pub fn new(config: AgentClientConfig) -> Self {
        let pi_path = config
            .endpoint
            .unwrap_or_else(|| "pi".to_string());
        let subprocess_client =
            PiSubprocessClient::new(pi_path, config.timeout_secs);
        Self {
            subprocess_client,
        }
    }
}

#[async_trait]
impl AgentExecutor for PiAgentClient {
    async fn execute(&self, prompt: &str) -> Result<AgentResponse, AgentError> {
        // Call pi CLI via subprocess
        let content = self.subprocess_client.execute(prompt).await?;

        // Return the response
        Ok(AgentResponse {
            content,
            done: true,
            usage: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pi_agent_client_default_config() {
        let config = AgentClientConfig::default();
        let client = PiAgentClient::new(config);
        // Verify the client was created successfully
        // (we can't easily test execution without pi installed)
        assert_eq!(client.subprocess_client.pi_path(), "pi");
    }

    #[test]
    fn test_pi_agent_client_custom_path() {
        let config = AgentClientConfig::new()
            .with_endpoint("/custom/path/to/pi".to_string());
        let client = PiAgentClient::new(config);
        assert_eq!(
            client.subprocess_client.pi_path(),
            "/custom/path/to/pi"
        );
    }
}
