//! Pi agent client for executing prompts

pub mod client;
pub mod response;
pub mod subprocess_client;
pub mod streaming;
pub mod pi_events;

use async_trait::async_trait;
pub use client::{AgentClientConfig};
pub use response::{AgentResponse, AgentError};
pub use subprocess_client::PiSubprocessClient;
pub use pi_events::PiJsonEvent;
pub use streaming::ProgressCallback;

/// Trait for agent execution - allows for different implementations
#[async_trait]
pub trait AgentExecutor: Send + Sync {
    /// Execute a prompt and return the full response
    async fn execute(&self, prompt: &str) -> Result<AgentResponse, AgentError>;

    /// Execute a prompt with streaming and optional progress callback
    ///
    /// This method executes the prompt and streams JSON events back to the caller
    /// via the optional callback. Text deltas are accumulated and returned in the
    /// final response.
    ///
    /// # Arguments
    /// * `prompt` - The prompt to send to the agent
    /// * `callback` - Optional callback to receive events as they arrive
    ///
    /// # Returns
    /// An `AgentResponse` with the accumulated text content
    ///
    /// # Example
    /// ```no_run
    /// # use pipeline::{AgentExecutor, PiAgentClient, AgentClientConfig};
    /// # use pipeline::agent::{ProgressCallback, PiJsonEvent};
    /// #
    /// struct MyCallback;
    ///
    /// impl ProgressCallback for MyCallback {
    ///     fn on_event(&self, event: &PiJsonEvent) {
    ///         match event {
    ///             PiJsonEvent::TextDelta { delta } => print!("{}", delta),
    ///             PiJsonEvent::AgentEnd => println!("\n[Done]"),
    ///             _ => {}
    ///         }
    ///     }
    /// }
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let config = AgentClientConfig::new();
    /// let client = PiAgentClient::new(config);
    /// let callback = MyCallback;
    /// let response = client.execute_streaming("Hello world", Some(&callback)).await?;
    /// # Ok(())
    /// # }
    /// ```
    async fn execute_streaming(
        &self,
        prompt: &str,
        callback: Option<&dyn ProgressCallback>,
    ) -> Result<AgentResponse, AgentError>;
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

    async fn execute_streaming(
        &self,
        prompt: &str,
        callback: Option<&dyn ProgressCallback>,
    ) -> Result<AgentResponse, AgentError> {
        // Delegate to subprocess client
        self.subprocess_client.execute_streaming(prompt, callback).await
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

    #[tokio::test]
    async fn test_pi_agent_client_implements_execute_streaming() {
        // Test that PiAgentClient implements execute_streaming
        let config = AgentClientConfig::default();
        let client: PiAgentClient = PiAgentClient::new(config);

        // This should compile - verifying the method exists
        // We'll test with a mock callback
        use crate::agent::streaming::NoopCallback;
        let callback = NoopCallback;

        // This won't work without pi installed, but we're testing compilation
        let _ = client.execute_streaming("test", Some(&callback));
    }
}
