//! Agent client configuration and implementation

/// Configuration for agent client
#[derive(Debug, Clone)]
pub struct AgentClientConfig {
    /// Path to pi executable or API endpoint
    ///
    /// When using subprocess client, this should be path to `pi` executable.
    /// If not provided, defaults to "pi" (assumes it's on PATH).
    pub endpoint: Option<String>,

    /// Timeout for requests in seconds
    pub timeout_secs: u64,
}

impl Default for AgentClientConfig {
    fn default() -> Self {
        Self {
            endpoint: None,
            timeout_secs: 10800,
        }
    }
}

impl AgentClientConfig {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self::default()
    }

    #[allow(dead_code)]
    pub fn with_endpoint(mut self, endpoint: String) -> Self {
        self.endpoint = Some(endpoint);
        self
    }

    #[allow(dead_code)]
    pub fn with_timeout(mut self, timeout_secs: u64) -> Self {
        self.timeout_secs = timeout_secs;
        self
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_client_config_builder() {
        let config = AgentClientConfig::new()
            .with_endpoint("http://localhost:8080".to_string())
            .with_timeout(600);

        assert_eq!(config.endpoint, Some("http://localhost:8080".to_string()));
        assert_eq!(config.timeout_secs, 600);
    }
}
