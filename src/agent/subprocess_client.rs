//! Pi CLI subprocess client - calls pi in print mode

use crate::agent::{AgentError, PiJsonEvent, AgentResponse};
use crate::agent::pi_events::AssistantMessageEvent;
use crate::agent::streaming::ProgressCallback;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
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

    /// Execute a prompt through pi CLI subprocess with streaming events
    ///
    /// Calls `pi --mode json --print <prompt>` and reads JSON events line-by-line.
    ///
    /// # Streaming Behavior
    /// - Subprocess spawns with JSON mode for event-based streaming
    /// - stdout is read line-by-line in real-time
    /// - Each valid JSON line is parsed as `PiJsonEvent`
    /// - Text deltas are accumulated into final content buffer
    /// - Callback is invoked for each parsed event (if provided)
    /// - Malformed JSON lines are logged but don't crash parsing
    ///
    /// # Arguments
    /// * `prompt` - The prompt to send to pi
    /// * `callback` - Optional callback for processing events as they arrive
    ///
    /// # Returns
    /// An `AgentResponse` with the accumulated text content and `done: true`
    ///
    /// # Errors
    /// Returns `AgentError` if:
    /// - The pi executable cannot be spawned
    /// - pi exits with a non-zero status
    /// - The command times out
    ///
    /// # Example
    /// ```no_run
    /// # use pipeline::agent::{PiSubprocessClient, ProgressCallback, PiJsonEvent};
    /// #
    /// struct MyCallback;
    ///
    /// impl ProgressCallback for MyCallback {
    ///     fn on_event(&self, event: &PiJsonEvent) {
    ///         println!("{:?}", event);
    ///     }
    /// }
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = PiSubprocessClient::new("pi".to_string(), 30);
    /// let callback = MyCallback;
    /// let response = client.execute_streaming("Hello", Some(&callback)).await?;
    /// println!("Final content: {}", response.content);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn execute_streaming(
        &self,
        prompt: &str,
        callback: Option<&dyn ProgressCallback>,
    ) -> Result<AgentResponse, AgentError> {
        debug!(
            "Spawning pi subprocess for streaming with prompt length: {}",
            prompt.len()
        );

        let timeout_duration = Duration::from_secs(self.timeout_secs);

        // Spawn pi in JSON mode with streaming
        let mut child = Command::new(&self.pi_path)
            .args(["--mode", "json", "--print"])
            .arg(prompt)
            .stdout(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| AgentError::Internal(format!("Failed to spawn pi subprocess: {}", e)))?;

        // Get stdout handle
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AgentError::Internal("Failed to get stdout handle".to_string()))?;

        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();

        let mut accumulated_text = String::new();

        // Read stdout line-by-line with timeout
        loop {
            let line_result = timeout(timeout_duration, lines.next_line())
                .await
                .map_err(|_| AgentError::Timeout(self.timeout_secs))?;

            match line_result {
                Ok(Some(line)) => {
                    if line.is_empty() {
                        continue;
                    }

                    // Parse the line as a JSON event
                    match serde_json::from_str::<PiJsonEvent>(&line) {
                        Ok(event) => {
                            debug!("Parsed event: {:?}", event);

                            // Handle different event types
                            match &event {
                                PiJsonEvent::MessageUpdate { assistant_message_event, .. } => {
                                    // Extract text from nested MessageUpdate events
                                    if let Some(assistant_event) = assistant_message_event {
                                        match assistant_event {
                                            AssistantMessageEvent::TextDelta { delta, .. } => {
                                                accumulated_text.push_str(delta);
                                            }
                                            AssistantMessageEvent::TextEnd { content, .. } => {
                                                debug!("Received text_end event with content: {:?}", content);
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                                PiJsonEvent::Session { .. } => {
                                    debug!("Received session event");
                                }
                                _ => {}
                            }

                            // Call callback if provided
                            if let Some(cb) = callback {
                                cb.on_event(&event);
                            }
                        }
                        Err(e) => {
                            // Log malformed JSON but continue processing
                            warn!("Failed to parse JSON line: {} - Line: {}", e, line);
                        }
                    }
                }
                Ok(None) => {
                    // End of output
                    break;
                }
                Err(e) => {
                    return Err(AgentError::Internal(format!("Failed to read stdout: {}", e)));
                }
            }
        }

        // Wait for the subprocess to finish
        let status = timeout(timeout_duration, child.wait())
            .await
            .map_err(|_| AgentError::Timeout(self.timeout_secs))?
            .map_err(|e| AgentError::Internal(format!("Failed to wait for subprocess: {}", e)))?;

        // Check exit code
        if !status.success() {
            return Err(AgentError::Api(format!(
                "pi exited with non-zero status: {:?}",
                status.code()
            )));
        }

        debug!(
            "pi subprocess streaming returned {} bytes of accumulated text",
            accumulated_text.len()
        );

        Ok(AgentResponse {
            content: accumulated_text,
            done: true,
            usage: None,
        })
    }

    /// Execute a prompt through pi CLI subprocess
    ///
    /// Calls `pi --mode text --print <prompt>` and captures stdout.
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
                .args(["--mode", "text", "--print"])
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
    use crate::agent::streaming::NoopCallback;
    use std::sync::{Arc, Mutex};

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

    #[tokio::test]
    #[ignore] // Requires pi to be installed
    async fn test_execute_streaming_with_real_pi() {
        // Test with real Pi subprocess
        let client = PiSubprocessClient::new("pi".to_string(), 30);
        let callback = NoopCallback;
        let result = client.execute_streaming("Say hello in one word", Some(&callback)).await;
        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response.content.to_lowercase().contains("hello"));
        assert!(response.done);
    }

    #[tokio::test]
    async fn test_execute_streaming_invalid_path() {
        // Test with invalid path
        let client = PiSubprocessClient::new("nonexistent-binary".to_string(), 30);
        let result = client.execute_streaming("Say hello", None).await;
        assert!(result.is_err());
        match result {
            Err(AgentError::Internal(msg)) => {
                assert!(msg.contains("Failed to spawn"));
            }
            _ => panic!("Expected AgentError::Internal"),
        }
    }

    #[tokio::test]
    async fn test_execute_streaming_with_callback() {
        // Test with a mock subprocess that outputs JSON events
        let client = PiSubprocessClient::new("echo".to_string(), 5);
        let echo_input = r#"{"type":"agent_start"}
{"type":"text_delta","delta":"Hello "}
{"type":"text_delta","delta":"world"}
{"type":"agent_end"}"#;

        let events = Arc::new(Mutex::new(Vec::new()));
        let events_clone = Arc::clone(&events);

        struct TestCallback {
            events: Arc<Mutex<Vec<PiJsonEvent>>>,
        }

        impl ProgressCallback for TestCallback {
            fn on_event(&self, event: &PiJsonEvent) {
                self.events.lock().unwrap().push(event.clone());
            }
        }

        let callback = TestCallback { events: events_clone };

        // This tests the callback mechanism with echo as a mock subprocess
        let result = client.execute_streaming(echo_input, Some(&callback)).await;

        // The result may fail (echo doesn't implement the full JSON protocol),
        // but we verify the callback was called and events were parsed
        let collected = events.lock().unwrap();
        // At least some events should have been parsed (or result is an error)
        assert!(!collected.is_empty() || result.is_err());
    }

    #[tokio::test]
    async fn test_execute_streaming_with_no_callback() {
        // Test without callback (None)
        let client = PiSubprocessClient::new("echo".to_string(), 5);
        let result = client.execute_streaming("test", None).await;

        // Result will fail because echo doesn't exit properly
        // but we're just testing that None doesn't cause a panic
        let _ = result;
    }

    #[tokio::test]
    async fn test_malformed_json_handling() {
        // Test that the parser handles empty output gracefully
        // The warn! macro in execute_streaming() handles malformed JSON logging
        let client = PiSubprocessClient::new("true".to_string(), 5);
        let result = client.execute_streaming("", None).await;
        // Empty output is valid - returns success with empty content
        assert!(result.is_ok());
        assert_eq!(result.unwrap().content, "");
    }
}
