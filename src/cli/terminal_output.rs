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
use console::style;
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
#[derive(Debug)]
pub struct TerminalOutputCallback {
    show_thinking: bool,
    #[allow(dead_code)]
    step_number: AtomicUsize,  // Reserved for Phase 3/4: step progress tracking
    #[allow(dead_code)]
    total_steps: AtomicUsize,  // Reserved for Phase 3/4: step progress tracking
    in_thinking: AtomicBool,
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
        }
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
            PiJsonEvent::TextDelta { delta } => {
                // Print text immediately and flush
                print!("{}", delta);
                self.flush_stdout();
            }
            PiJsonEvent::TextEnd { .. } => {
                // Text output complete - ensure newline
                println!();
            }
            PiJsonEvent::ThinkingDelta { .. } => {
                // Start thinking section if not already in it
                if self.show_thinking && !self.in_thinking.load(Ordering::SeqCst) {
                    self.print_thinking_start();
                }
                // Thinking deltas are printed only if show_thinking is enabled
                // For now, we skip them (can be enabled later for debugging)
            }
            PiJsonEvent::ThinkingEnd { .. } => {
                self.print_thinking_end();
            }
            PiJsonEvent::AgentEnd => {
                // Pipeline execution complete
                println!();
            }
            // Other event types will be handled in Phase 3 (tool calls)
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_text_delta_event_does_not_crash() {
        let callback = TerminalOutputCallback::new(false, 1);

        // Should not panic
        callback.on_event(&PiJsonEvent::TextDelta {
            delta: "Hello, ".to_string(),
        });
        callback.on_event(&PiJsonEvent::TextDelta {
            delta: "world!".to_string(),
        });
        callback.on_event(&PiJsonEvent::TextEnd {
            content: Some("Hello, world!".to_string()),
        });
    }

    #[test]
    fn test_agent_start_end_events_do_not_crash() {
        let callback = TerminalOutputCallback::new(false, 1);

        callback.on_event(&PiJsonEvent::AgentStart);
        callback.on_event(&PiJsonEvent::AgentEnd);
        // Should not panic
    }

    #[test]
    fn test_thinking_delta_with_show_thinking_false() {
        let callback = TerminalOutputCallback::new(false, 1);

        // Should not start thinking section
        callback.on_event(&PiJsonEvent::ThinkingDelta {
            delta: "Thinking...".to_string(),
        });

        // Should not be in thinking mode
        assert_eq!(callback.in_thinking.load(Ordering::SeqCst), false);
    }

    #[test]
    fn test_thinking_delta_with_show_thinking_true() {
        let callback = TerminalOutputCallback::new(true, 1);

        // Should start thinking section
        callback.on_event(&PiJsonEvent::ThinkingDelta {
            delta: "Thinking...".to_string(),
        });

        // Should be in thinking mode
        assert_eq!(callback.in_thinking.load(Ordering::SeqCst), true);
    }

    #[test]
    fn test_thinking_end_closes_section() {
        let callback = TerminalOutputCallback::new(true, 1);

        // Start thinking
        callback.on_event(&PiJsonEvent::ThinkingDelta {
            delta: "Thinking...".to_string(),
        });
        assert_eq!(callback.in_thinking.load(Ordering::SeqCst), true);

        // End thinking
        callback.on_event(&PiJsonEvent::ThinkingEnd {
            content: Some("Done thinking".to_string()),
        });
        assert_eq!(callback.in_thinking.load(Ordering::SeqCst), false);
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
