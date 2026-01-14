//! Pi CLI subprocess client - calls pi in print mode

use crate::agent::AgentError;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;
use tracing::{debug, warn};

/// Client for executing Pi CLI as a subprocess
#[derive(Debug, Clone)]
pub struct PiSubprocessClient {
    /// Path to pi executable
    pi_path: String,

    /// Timeout for command execution in seconds
    timeout_secs: u64,
}

impl PiSubprocessClient {
    /// Create a new subprocess client
    ///
    /// # Arguments
    /// * `pi_path` - Path to pi executable (e.g., "pi", "/usr/local/bin/pi")
    /// * `timeout_secs` - Timeout for command execution in seconds
    pub fn new(pi_path: String, timeout_secs: u64) -> Self {
        Self {
            pi_path,
            timeout_secs,
        }
    }

    /// Get the pi executable path
    #[cfg(test)]
    pub fn pi_path(&self) -> &str {
        &self.pi_path
    }

    /// Execute a prompt through pi CLI subprocess
    ///
    /// Calls `pi --mode text --print --no-session <prompt>` and captures stdout.
    ///
    /// # Arguments
    /// * `prompt` - The prompt to send to pi
    ///
    /// # Returns
    /// The full response text from pi
    ///
    /// # Errors
    /// Returns `AgentError` if:
    /// - The pi executable cannot be spawned
    /// - pi exits with a non-zero status
    /// - The output is not valid UTF-8
    /// - The command times out
    pub async fn execute(&self, prompt: &str) -> Result<String, AgentError> {
        debug!("Spawning pi subprocess with prompt length: {}", prompt.len());

        let timeout_duration = Duration::from_secs(self.timeout_secs);

        // Spawn pi in text/print mode
        let result = timeout(
            timeout_duration,
            Command::new(&self.pi_path)
                .args(["--mode", "text", "--print", "--no-session"])
                .arg(prompt)
                .kill_on_drop(true)
                .output(),
        )
        .await
        .map_err(|_| AgentError::Timeout(self.timeout_secs))?;

        let output = result.map_err(|e| {
            AgentError::Internal(format!("Failed to execute pi subprocess: {}", e))
        })?;

        // Check exit code
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let exit_code = output.status.code().unwrap_or(-1);
            warn!(
                "pi exited with code {}: {}",
                exit_code,
                stderr.trim()
            );
            return Err(AgentError::Api(format!(
                "pi exited with code {}: {}",
                exit_code,
                stderr.trim()
            )));
        }

        // Stdout IS the response (text mode)
        let content = String::from_utf8(output.stdout).map_err(|e| {
            AgentError::Internal(format!("Failed to decode pi output: {}", e))
        })?;

        debug!("pi subprocess returned {} bytes of output", content.len());

        Ok(content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires pi to be installed
    async fn test_subprocess_hello() {
        let client = PiSubprocessClient::new("pi".to_string(), 30);
        let result = client.execute("Say hello in one word").await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.to_lowercase().contains("hello"));
    }

    #[tokio::test]
    #[ignore]
    async fn test_subprocess_timeout() {
        let client = PiSubprocessClient::new("pi".to_string(), 1);
        let result = client
            .execute("Wait 10 seconds then say done")
            .await;
        assert!(matches!(result, Err(AgentError::Timeout(_))));
    }

    #[tokio::test]
    #[ignore]
    async fn test_subprocess_invalid_path() {
        let client = PiSubprocessClient::new("nonexistent-pi-binary".to_string(), 30);
        let result = client.execute("Say hello").await;
        assert!(result.is_err());
    }
}
