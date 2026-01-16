//! Terminal output callback for streaming agent execution
//!
//! This module provides the `TerminalOutputCallback` implementation that
//! displays streaming events from the agent in real-time to the terminal.
//!
//! # Features
//!
//! - Step headers with progress indicators: `[1/3] Planning`
//! - Real-time text delta display
//! - Horizontal separators between steps
//! - Optional thinking display (controlled by `--show-thinking` flag)
//! - Color-coded tool call indicators:
//!   - `<read: path>` in blue
//!   - `<write: path>` in green
//!   - `<bash: command>` in yellow
//!   - `<edit: path>` in cyan
//! - Tool execution status with ✓ (success) and ✗ (error)
//! - Stdout flushing for immediate output
//!
//! # Example
//!
//! ```no_run
//! use pipeline::cli::terminal_output::TerminalOutputCallback;
//! use pipeline::agent::ProgressCallback;
//! use pipeline::agent::PiJsonEvent;
//!
//! let callback = TerminalOutputCallback::new(true, 3);
//! callback.on_event(&PiJsonEvent::AgentStart);
//! ```

use crate::agent::{ProgressCallback, PiJsonEvent};
use crate::agent::pi_events::AssistantMessageEvent;
use console::style;
use serde_json::Value;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::io::{self, Write};

/// Callback that displays streaming events to the terminal
///
/// This callback prints events as they arrive from the agent subprocess,
/// providing real-time visibility into the agent's progress.
///
/// # Fields
///
/// * `show_thinking` - Whether to display thinking deltas (verbose output)
/// * `step_number` - Current step number (for header display) - RESERVED FOR FUTURE USE (Phase 3/4)
/// * `total_steps` - Total number of steps (for header display) - RESERVED FOR FUTURE USE (Phase 3/4)
/// * `in_thinking` - Whether currently inside a thinking section
/// * `last_tool_call_id` - Last tool call ID for matching with execution events
#[derive(Debug)]
pub struct TerminalOutputCallback {
    show_thinking: bool,
    #[allow(dead_code)]
    step_number: AtomicUsize,  // Reserved for Phase 3/4: step progress tracking
    #[allow(dead_code)]
    total_steps: AtomicUsize,  // Reserved for Phase 3/4: step progress tracking
    in_thinking: AtomicBool,
    last_tool_call_id: AtomicUsize,  // Last tool call index for validation
}

impl TerminalOutputCallback {
    /// Create a new terminal output callback
    ///
    /// # Arguments
    ///
    /// * `show_thinking` - Whether to display thinking deltas (reasoning output)
    /// * `total_steps` - Total number of steps for progress display
    ///
    /// # Example
    ///
    /// ```
    /// use pipeline::cli::terminal_output::TerminalOutputCallback;
    ///
    /// let callback = TerminalOutputCallback::new(true, 3);
    /// ```
    pub fn new(show_thinking: bool, total_steps: usize) -> Self {
        Self {
            show_thinking,
            step_number: AtomicUsize::new(0),
            total_steps: AtomicUsize::new(total_steps),
            in_thinking: AtomicBool::new(false),
            last_tool_call_id: AtomicUsize::new(0),
        }
    }

    /// Get color style for a tool type
    ///
    /// # Color Mapping
    ///
    /// - `read` → blue
    /// - `write` → green
    /// - `bash` → yellow
    /// - `edit` → cyan
    /// - other → white
    ///
    /// # Example
    ///
    /// ```
    /// use pipeline::cli::terminal_output::TerminalOutputCallback;
    /// let _color = TerminalOutputCallback::get_tool_color("read");
    /// ```
    pub fn get_tool_color(tool_name: &str) -> String {
        match tool_name {
            "read" => "\x1b[34m",  // Blue
            "write" => "\x1b[32m", // Green
            "bash" => "\x1b[33m",  // Yellow
            "edit" => "\x1b[36m",  // Cyan
            _ => "\x1b[37m",      // White
        }.to_string()
    }

    /// Get reset ANSI code
    fn get_reset_color() -> String {
        "\x1b[0m".to_string()
    }

    /// Get success indicator color (green)
    pub fn get_success_color() -> String {
        "\x1b[32m".to_string() // Green
    }

    /// Get error indicator color (red)
    pub fn get_error_color() -> String {
        "\x1b[31m".to_string() // Red
    }

    /// Extract a string argument value from JSON arguments
    ///
    /// Returns the argument value as a string if it exists and is a string,
    /// otherwise returns an empty string.
    fn extract_arg_value(args: &Value, arg_name: &str) -> String {
        args.get(arg_name)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    }

    /// Format tool arguments for display
    ///
    /// Extracts and formats relevant arguments based on tool type:
    ///
    /// - `read`: `path` argument
    /// - `write`: `path` and optionally `content` (truncated)
    /// - `bash`: `command` argument
    /// - `edit`: `path`, `oldText` preview (first line), `newText` preview (first line)
    ///
    /// Long arguments (> 50 chars) are truncated with "..."
    fn format_tool_args(tool_name: &str, args: &Value) -> String {
        match tool_name {
            "read" => {
                let path = Self::extract_arg_value(args, "path");
                Self::truncate_string(&path, 50)
            }
            "write" => {
                let path = Self::extract_arg_value(args, "path");
                Self::truncate_string(&path, 50)
            }
            "bash" => {
                let command = Self::extract_arg_value(args, "command");
                Self::truncate_string(&command, 50)
            }
            "edit" => {
                let path = Self::extract_arg_value(args, "path");
                let old_text = Self::extract_arg_value(args, "oldText");
                let new_text = Self::extract_arg_value(args, "newText");

                // Get first line of old/new text for preview
                let old_preview = old_text.lines().next().unwrap_or("");
                let new_preview = new_text.lines().next().unwrap_or("");

                // Truncate previews
                let old_truncated = Self::truncate_string(old_preview, 30);
                let new_truncated = Self::truncate_string(new_preview, 30);

                format!("{} | old: \"{}\" | new: \"{}\"",
                    Self::truncate_string(&path, 50),
                    old_truncated,
                    new_truncated
                )
            }
            _ => {
                // For unknown tools, try to extract "path" or "command"
                let path = Self::extract_arg_value(args, "path");
                if !path.is_empty() {
                    Self::truncate_string(&path, 50)
                } else {
                    let command = Self::extract_arg_value(args, "command");
                    if !command.is_empty() {
                        Self::truncate_string(&command, 50)
                    } else {
                        "unknown".to_string()
                    }
                }
            }
        }
    }

    /// Truncate a string to a maximum length
    ///
    /// If the string is longer than max_len, truncates and adds "..."
    fn truncate_string(s: &str, max_len: usize) -> String {
        if s.len() <= max_len {
            s.to_string()
        } else {
            format!("{}...", &s[..max_len.saturating_sub(3)])
        }
    }

    /// Format a tool call for display
    ///
    /// Returns a formatted string like `<read: src/file.rs>` with color coding
    pub fn format_tool_call(tool_name: &str, args: &Value) -> String {
        let args_str = Self::format_tool_args(tool_name, args);
        let color = Self::get_tool_color(tool_name);
        let reset = Self::get_reset_color();
        format!("{}<{}: {}>{}", color, tool_name, args_str, reset)
    }

    /// Format a tool execution result for display
    ///
    /// Shows ✓ for success or ✗ for error with result summary
    pub fn format_tool_result(is_error: bool, result: &Value) -> String {
        let indicator = if is_error { "✗" } else { "✓" };
        let color = if is_error {
            Self::get_error_color()
        } else {
            Self::get_success_color()
        };
        let reset = Self::get_reset_color();

        // Extract result summary
        let summary = Self::extract_result_summary(result);
        format!("{}{} {}{}", color, indicator, summary, reset)
    }

    /// Extract a summary from the result value
    ///
    /// For text results, shows first line or truncated output
    fn extract_result_summary(result: &Value) -> String {
        // Try to extract text content from result
        if let Some(content_array) = result.get("content").and_then(|v| v.as_array()) {
            if let Some(first_content) = content_array.first() {
                if let Some(text) = first_content.get("text").and_then(|v| v.as_str()) {
                    // Take first line and truncate
                    let first_line = text.lines().next().unwrap_or("");
                    return Self::truncate_string(first_line, 100);
                }
            }
        }

        // Fallback: try to get any string value
        if let Some(s) = result.as_str() {
            return Self::truncate_string(s, 100);
        }

        // Default summary
        "completed".to_string()
    }

    /// Print a step header
    ///
    /// Format: `[N/M] Step Name`
    ///
    /// # Arguments
    ///
    /// * `step_num` - Current step number (1-indexed)
    /// * `step_name` - Name of the step
    ///
    /// **Note:** RESERVED FOR FUTURE USE - Will be called in Phase 3/4 to display step progress
    #[allow(dead_code)]
    fn print_step_header(&self, step_num: usize, step_name: &str) {
        let total = self.total_steps.load(Ordering::SeqCst);
        println!(
            "\n[{} / {}] {}\n",
            style(step_num).cyan(),
            style(total).dim(),
            style(step_name).bold()
        );
    }

    /// Print a separator line
    ///
    /// A horizontal rule spanning the terminal width, used to visually
    /// separate different sections of output.
    ///
    /// **Note:** RESERVED FOR FUTURE USE - Will be called between steps in Phase 3/4
    #[allow(dead_code)]
    fn print_separator(&self) {
        // Get terminal width, default to 80 if unavailable
        let width = term_size::dimensions_stdout()
            .map(|(w, _)| w)
            .unwrap_or(80);
        println!("{}", "─".repeat(width));
    }

    /// Flush stdout to ensure immediate display
    ///
    /// This is called after each print operation to ensure the output
    /// appears immediately without buffering delays.
    fn flush_stdout(&self) {
        let _ = io::stdout().flush();
    }

    /// Print thinking section start
    ///
    /// Called when thinking begins and `show_thinking` is enabled.
    fn print_thinking_start(&self) {
        if self.show_thinking {
            println!("{}", style("Thinking...").dim());
            self.in_thinking.store(true, Ordering::SeqCst);
        }
    }

    /// Print thinking section end
    ///
    /// Called when thinking completes and `show_thinking` is enabled.
    fn print_thinking_end(&self) {
        if self.show_thinking && self.in_thinking.load(Ordering::SeqCst) {
            println!("{}", style("End of thinking").dim());
            self.in_thinking.store(false, Ordering::SeqCst);
        }
    }

    /// Increment the step number
    ///
    /// This is called to advance to the next step, used for progress display.
    ///
    /// **Note:** RESERVED FOR FUTURE USE - Will be used for step progress tracking in Phase 3/4
    #[allow(dead_code)]
    fn increment_step(&self) {
        self.step_number.fetch_add(1, Ordering::SeqCst);
    }
}

impl ProgressCallback for TerminalOutputCallback {
    fn on_event(&self, event: &PiJsonEvent) {
        match event {
            PiJsonEvent::AgentStart => {
                // Pipeline execution started - could print header here
            }
            PiJsonEvent::MessageUpdate { assistant_message_event, .. } => {
                if let Some(assistant_event) = assistant_message_event {
                    match assistant_event {
                        AssistantMessageEvent::TextDelta { delta, .. } => {
                            // Print text immediately and flush
                            print!("{}", delta);
                            self.flush_stdout();
                        }
                        AssistantMessageEvent::TextEnd { .. } => {
                            // Text output complete - ensure newline
                            println!();
                        }
                        AssistantMessageEvent::ThinkingDelta { .. } => {
                            // Start thinking section if not already in it
                            if self.show_thinking && !self.in_thinking.load(Ordering::SeqCst) {
                                self.print_thinking_start();
                            }
                            // Thinking deltas are printed only if show_thinking is enabled
                        }
                        AssistantMessageEvent::ThinkingEnd { .. } => {
                            self.print_thinking_end();
                        }
                        AssistantMessageEvent::ToolcallStart { partial, .. } => {
                            // Extract tool name from the partial message content
                            if let Some(content_array) = partial.content.get(0) {
                                if let Some(tool_name) = content_array.get("name").and_then(|v| v.as_str()) {
                                    if let Some(args) = content_array.get("arguments") {
                                        println!();
                                        println!("{}", Self::format_tool_call(tool_name, args));
                                        self.flush_stdout();
                                    }
                                }
                            }
                        }
                        AssistantMessageEvent::ToolcallEnd { tool_call, .. } => {
                            // Tool call complete - store tool_call_id for result matching
                            // We increment a counter to track tool call sequence
                            self.last_tool_call_id.fetch_add(1, Ordering::SeqCst);
                        }
                        _ => {}
                    }
                }
            }
            PiJsonEvent::ToolExecutionStart { tool_call_id, tool_name, .. } => {
                // Tool execution started
                // Validate that we have a matching tool call (basic check)
                let expected_count = self.last_tool_call_id.load(Ordering::SeqCst);
                if expected_count == 0 {
                    // No tool call was recorded - this might indicate an event stream issue
                    eprintln!();
                    eprintln!("  [Warning: Tool execution started without preceding tool call for '{}']",
                        tool_name);
                }
                println!("  Executing {}...", tool_name);
                self.flush_stdout();
            }
            PiJsonEvent::ToolExecutionEnd { tool_call_id, tool_name, is_error, result, .. } => {
                // Tool execution complete with result
                // Note: tool_call_id validation would require full ID storage and lookup
                // For now, we rely on event order as provided by the subprocess
                println!("  {}", Self::format_tool_result(*is_error, result));
                self.flush_stdout();
            }
            PiJsonEvent::AgentEnd => {
                // Pipeline execution complete
                println!();
            }
            // Other event types (Session, etc.) are ignored for terminal output
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // Phase 3: Tool Call Formatting Tests

    #[test]
    fn test_get_tool_color_for_read() {
        let color = TerminalOutputCallback::get_tool_color("read");
        // Blue for read - ANSI escape code
        assert_eq!(color, "\x1b[34m");
    }

    #[test]
    fn test_get_tool_color_for_write() {
        let _color = TerminalOutputCallback::get_tool_color("write");
        // Green for write
    }

    #[test]
    fn test_get_tool_color_for_bash() {
        let _color = TerminalOutputCallback::get_tool_color("bash");
        // Yellow for bash
    }

    #[test]
    fn test_get_tool_color_for_edit() {
        let _color = TerminalOutputCallback::get_tool_color("edit");
        // Cyan for edit
    }

    #[test]
    fn test_get_tool_color_for_unknown_tool() {
        let _color = TerminalOutputCallback::get_tool_color("unknown");
        // White for unknown
    }

    #[test]
    fn test_get_success_color() {
        let _color = TerminalOutputCallback::get_success_color();
        // Green
    }

    #[test]
    fn test_get_error_color() {
        let _color = TerminalOutputCallback::get_error_color();
        // Red
    }

    #[test]
    fn test_extract_arg_value_with_string() {
        let args = json!({"path": "src/file.rs"});
        let value = TerminalOutputCallback::extract_arg_value(&args, "path");
        assert_eq!(value, "src/file.rs");
    }

    #[test]
    fn test_extract_arg_value_missing() {
        let args = json!({"path": "src/file.rs"});
        let value = TerminalOutputCallback::extract_arg_value(&args, "command");
        assert_eq!(value, "");
    }

    #[test]
    fn test_extract_arg_value_non_string() {
        let args = json!({"count": 42});
        let value = TerminalOutputCallback::extract_arg_value(&args, "count");
        assert_eq!(value, "");
    }

    #[test]
    fn test_truncate_string_short() {
        let result = TerminalOutputCallback::truncate_string("hello", 50);
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_truncate_string_exact() {
        let result = TerminalOutputCallback::truncate_string("hello", 5);
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_truncate_string_long() {
        let result = TerminalOutputCallback::truncate_string("hello world", 8);
        assert_eq!(result, "hello...");
    }

    #[test]
    fn test_format_tool_args_read() {
        let args = json!({"path": "src/auth/auth.rs"});
        let result = TerminalOutputCallback::format_tool_args("read", &args);
        assert_eq!(result, "src/auth/auth.rs");
    }

    #[test]
    fn test_format_tool_args_write() {
        let args = json!({"path": "src/main.rs", "content": "code here"});
        let result = TerminalOutputCallback::format_tool_args("write", &args);
        assert_eq!(result, "src/main.rs");
    }

    #[test]
    fn test_format_tool_args_bash() {
        let args = json!({"command": "cargo build"});
        let result = TerminalOutputCallback::format_tool_args("bash", &args);
        assert_eq!(result, "cargo build");
    }

    #[test]
    fn test_format_tool_args_edit() {
        let args = json!({"path": "src/file.rs", "oldText": "old", "newText": "new"});
        let result = TerminalOutputCallback::format_tool_args("edit", &args);
        assert!(result.contains("src/file.rs"));
        assert!(result.contains("old: \"old\""));
        assert!(result.contains("new: \"new\""));
    }

    #[test]
    fn test_format_tool_args_edit_multiline() {
        let args = json!({
            "path": "src/file.rs",
            "oldText": "line1\nline2\nline3",
            "newText": "new line1\nnew line2"
        });
        let result = TerminalOutputCallback::format_tool_args("edit", &args);
        assert!(result.contains("src/file.rs"));
        assert!(result.contains("old: \"line1\""));  // First line only
        assert!(result.contains("new: \"new line1\""));  // First line only
        assert!(!result.contains("line2"));  // Second line should not appear
    }

    #[test]
    fn test_format_tool_args_edit_long_text() {
        let long_text = "a".repeat(50);
        let args = json!({
            "path": "src/file.rs",
            "oldText": long_text.as_str(),
            "newText": long_text.as_str()
        });
        let result = TerminalOutputCallback::format_tool_args("edit", &args);
        // Text should be truncated to 30 chars with "..."
        assert!(result.contains("..."));  // Should contain truncation indicator
        // Old/new text should not be the full 50 characters
        assert!(!result.contains(&format!("old: \"{}\"", long_text)));  // Not full text
    }

    #[test]
    fn test_format_tool_args_edit_empty_text() {
        let args = json!({
            "path": "src/file.rs",
            "oldText": "",
            "newText": ""
        });
        let result = TerminalOutputCallback::format_tool_args("edit", &args);
        assert!(result.contains("src/file.rs"));
        assert!(result.contains("old: \"\""));
        assert!(result.contains("new: \"\""));
    }

    #[test]
    fn test_format_tool_args_edit_missing_text() {
        let args = json!({"path": "src/file.rs"});
        let result = TerminalOutputCallback::format_tool_args("edit", &args);
        assert!(result.contains("src/file.rs"));
        // Should handle missing oldText/newText gracefully
    }

    #[test]
    fn test_format_tool_args_long_path() {
        let long_path = "src/very/long/path/that/exceeds/the/fifty/character/limit/file.rs";
        let args = json!({"path": long_path});
        let result = TerminalOutputCallback::format_tool_args("read", &args);
        assert_eq!(result.len(), 50);
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_format_tool_call_read() {
        let args = json!({"path": "src/auth/auth.rs"});
        let result = TerminalOutputCallback::format_tool_call("read", &args);
        assert!(result.contains("<read: src/auth/auth.rs>"));
    }

    #[test]
    fn test_format_tool_call_write() {
        let args = json!({"path": "package.json"});
        let result = TerminalOutputCallback::format_tool_call("write", &args);
        assert!(result.contains("<write: package.json>"));
    }

    #[test]
    fn test_format_tool_call_bash() {
        let args = json!({"command": "npm install"});
        let result = TerminalOutputCallback::format_tool_call("bash", &args);
        assert!(result.contains("<bash: npm install>"));
    }

    #[test]
    fn test_format_tool_call_edit() {
        let args = json!({"path": "src/file.rs"});
        let result = TerminalOutputCallback::format_tool_call("edit", &args);
        // Edit tool format now includes separators and old/new text previews
        assert!(result.contains("<edit:"));
        assert!(result.contains("src/file.rs"));
    }

    #[test]
    fn test_extract_result_summary_from_text() {
        let result = json!({"content": [{"type": "text", "text": "Read 156 lines"}]});
        let summary = TerminalOutputCallback::extract_result_summary(&result);
        assert_eq!(summary, "Read 156 lines");
    }

    #[test]
    fn test_extract_result_summary_multiline() {
        let text = "First line\nSecond line\nThird line";
        let result = json!({"content": [{"type": "text", "text": text}]});
        let summary = TerminalOutputCallback::extract_result_summary(&result);
        assert_eq!(summary, "First line");
    }

    #[test]
    fn test_extract_result_summary_long() {
        let long_text = "a".repeat(150);
        let result = json!({"content": [{"type": "text", "text": long_text.as_str()}]});
        let summary = TerminalOutputCallback::extract_result_summary(&result);
        assert_eq!(summary.len(), 100);
        assert!(summary.ends_with("..."));
    }

    #[test]
    fn test_extract_result_summary_from_string() {
        let result = json!("Simple string result");
        let summary = TerminalOutputCallback::extract_result_summary(&result);
        assert_eq!(summary, "Simple string result");
    }

    #[test]
    fn test_extract_result_summary_default() {
        let result = json!({"other": "value"});
        let summary = TerminalOutputCallback::extract_result_summary(&result);
        assert_eq!(summary, "completed");
    }

    #[test]
    fn test_format_tool_result_success() {
        let result = json!({"content": [{"type": "text", "text": "File written"}]});
        let formatted = TerminalOutputCallback::format_tool_result(false, &result);
        assert!(formatted.contains("✓"));
        assert!(formatted.contains("File written"));
    }

    #[test]
    fn test_format_tool_result_error() {
        let result = json!({"content": [{"type": "text", "text": "File not found"}]});
        let formatted = TerminalOutputCallback::format_tool_result(true, &result);
        assert!(formatted.contains("✗"));
        assert!(formatted.contains("File not found"));
    }

    #[test]
    fn test_format_tool_result_long_truncated() {
        let long_text = "a".repeat(150);
        let result = json!({"content": [{"type": "text", "text": long_text.as_str()}]});
        let formatted = TerminalOutputCallback::format_tool_result(false, &result);
        assert!(formatted.contains("✓"));
        // Result summary should be truncated (includes "...")
        assert!(formatted.contains("..."));
    }

    #[test]
    fn test_format_tool_result_null() {
        let result = json!(null);
        let formatted = TerminalOutputCallback::format_tool_result(false, &result);
        assert!(formatted.contains("✓"));
        assert!(formatted.contains("completed"));
    }

    // Phase 3: Event Handling Tests

    #[test]
    fn test_toolcall_start_event_does_not_panic() {
        let callback = TerminalOutputCallback::new(false, 1);

        // Create a ToolcallStart event
        let partial_message = crate::agent::pi_events::Message {
            role: "assistant".to_string(),
            content: vec![json!({
                "type": "toolCall",
                "id": "call_123",
                "name": "read",
                "arguments": {"path": "src/file.rs"}
            })],
        };

        let event = PiJsonEvent::MessageUpdate {
            assistant_message_event: Some(AssistantMessageEvent::ToolcallStart {
                content_index: 0,
                partial: partial_message,
            }),
            message: None,
        };

        // Should not panic
        callback.on_event(&event);
    }

    #[test]
    fn test_toolcall_end_event_does_not_panic() {
        let callback = TerminalOutputCallback::new(false, 1);

        let tool_call = crate::agent::pi_events::ToolCall {
            tool_type: "toolCall".to_string(),
            id: "call_123".to_string(),
            name: "read".to_string(),
            arguments: json!({"path": "src/file.rs"}),
        };

        let partial_message = crate::agent::pi_events::Message {
            role: "assistant".to_string(),
            content: vec![json!({
                "type": "toolCall",
                "id": "call_123",
                "name": "read",
                "arguments": {"path": "src/file.rs"}
            })],
        };

        let event = PiJsonEvent::MessageUpdate {
            assistant_message_event: Some(AssistantMessageEvent::ToolcallEnd {
                content_index: 0,
                tool_call,
                partial: partial_message,
            }),
            message: None,
        };

        // Should not panic
        callback.on_event(&event);
    }

    #[test]
    fn test_tool_execution_start_event_does_not_panic() {
        let callback = TerminalOutputCallback::new(false, 1);

        let event = PiJsonEvent::ToolExecutionStart {
            tool_call_id: "call_123".to_string(),
            tool_name: "bash".to_string(),
            args: json!({"command": "ls"}),
        };

        // Should not panic
        callback.on_event(&event);
    }

    #[test]
    fn test_tool_execution_end_success_event_does_not_panic() {
        let callback = TerminalOutputCallback::new(false, 1);

        let event = PiJsonEvent::ToolExecutionEnd {
            tool_call_id: "call_123".to_string(),
            tool_name: "bash".to_string(),
            result: json!({"content": [{"type": "text", "text": "completed"}]}),
            is_error: false,
        };

        // Should not panic
        callback.on_event(&event);
    }

    #[test]
    fn test_tool_execution_end_error_event_does_not_panic() {
        let callback = TerminalOutputCallback::new(false, 1);

        let event = PiJsonEvent::ToolExecutionEnd {
            tool_call_id: "call_123".to_string(),
            tool_name: "bash".to_string(),
            result: json!({"content": [{"type": "text", "text": "command failed"}]}),
            is_error: true,
        };

        // Should not panic
        callback.on_event(&event);
    }

    // Original tests from Phase 2

    #[test]
    fn test_callback_initialization_with_thinking_true() {
        let callback = TerminalOutputCallback::new(true, 3);

        // Verify fields are set correctly
        assert_eq!(callback.total_steps.load(Ordering::SeqCst), 3);
        assert_eq!(callback.in_thinking.load(Ordering::SeqCst), false);
    }

    #[test]
    fn test_callback_initialization_with_thinking_false() {
        let callback = TerminalOutputCallback::new(false, 5);

        assert_eq!(callback.total_steps.load(Ordering::SeqCst), 5);
        assert_eq!(callback.in_thinking.load(Ordering::SeqCst), false);
    }

    #[test]
    fn test_callback_initialization_different_total_steps() {
        let callback1 = TerminalOutputCallback::new(false, 1);
        let callback2 = TerminalOutputCallback::new(false, 10);
        let callback3 = TerminalOutputCallback::new(false, 100);

        assert_eq!(callback1.total_steps.load(Ordering::SeqCst), 1);
        assert_eq!(callback2.total_steps.load(Ordering::SeqCst), 10);
        assert_eq!(callback3.total_steps.load(Ordering::SeqCst), 100);
    }

    #[test]
    fn test_increment_step() {
        let callback = TerminalOutputCallback::new(false, 3);

        assert_eq!(callback.step_number.load(Ordering::SeqCst), 0);

        callback.increment_step();
        assert_eq!(callback.step_number.load(Ordering::SeqCst), 1);

        callback.increment_step();
        assert_eq!(callback.step_number.load(Ordering::SeqCst), 2);

        callback.increment_step();
        assert_eq!(callback.step_number.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn test_separator_formatting() {
        let callback = TerminalOutputCallback::new(false, 1);

        // Should not panic - width defaults to 80 if unavailable
        callback.print_separator();
    }

    #[test]
    fn test_step_header_formatting() {
        let callback = TerminalOutputCallback::new(false, 3);

        // Should not panic
        callback.print_step_header(1, "Planning");
        callback.print_step_header(2, "Implementation");
        callback.print_step_header(3, "Testing");
    }
}
