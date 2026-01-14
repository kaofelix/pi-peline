//! Agent client configuration and implementation

use crate::agent::PiAgentClient;

/// Configuration for the agent client
#[derive(Debug, Clone)]
pub struct AgentClientConfig {
    /// Path to pi executable or API endpoint
    ///
    /// When using the subprocess client, this should be the path to the `pi` executable.
    /// If not provided, defaults to "pi" (assumes it's on PATH).
    pub endpoint: Option<String>,

    /// API key (if applicable)
    pub api_key: Option<String>,

    /// Model to use (if applicable)
    pub model: Option<String>,

    /// Timeout for requests in seconds
    pub timeout_secs: u64,

    /// Whether to enable streaming responses (not yet supported in subprocess mode)
    pub enable_streaming: bool,
}

impl Default for AgentClientConfig {
    fn default() -> Self {
        Self {
            endpoint: None,
            api_key: None,
            model: None,
            timeout_secs: 300,
            enable_streaming: true,
        }
    }
}

impl AgentClientConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_endpoint(mut self, endpoint: String) -> Self {
        self.endpoint = Some(endpoint);
        self
    }

    pub fn with_api_key(mut self, api_key: String) -> Self {
        self.api_key = Some(api_key);
        self
    }

    pub fn with_model(mut self, model: String) -> Self {
        self.model = Some(model);
        self
    }

    pub fn with_timeout(mut self, timeout_secs: u64) -> Self {
        self.timeout_secs = timeout_secs;
        self
    }

    pub fn with_streaming(mut self, enable: bool) -> Self {
        self.enable_streaming = enable;
        self
    }
}

/// Create a default agent client
///
/// # Arguments
/// * `config` - Agent client configuration
///
/// # Example
/// ```ignore
/// use pipeline::agent::{create_agent_client, AgentClientConfig};
///
/// let client = create_agent_client(AgentClientConfig::new());
/// ```
pub fn create_agent_client(config: AgentClientConfig) -> PiAgentClient {
    PiAgentClient::new(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_client_config_builder() {
        let config = AgentClientConfig::new()
            .with_endpoint("http://localhost:8080".to_string())
            .with_api_key("test-key".to_string())
            .with_timeout(600);

        assert_eq!(config.endpoint, Some("http://localhost:8080".to_string()));
        assert_eq!(config.api_key, Some("test-key".to_string()));
        assert_eq!(config.timeout_secs, 600);
    }
}
