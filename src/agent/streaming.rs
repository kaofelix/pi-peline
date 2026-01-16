//! Streaming support for agent execution
//!
//! This module provides the callback mechanism for processing streaming events
//! from the Pi CLI agent during prompt execution.
//!
//! # Overview
//!
//! When using `execute_streaming()` on an `AgentExecutor`, JSON events are
//! parsed line-by-line from the subprocess stdout. Each event is delivered
//! to a callback for real-time processing.
//!
//! # Event Types
//!
//! See [`PiJsonEvent`] for all possible event types:
//! - `AgentStart` - Execution started
//! - `TextDelta` - Token of text output (most common)
//! - `TextEnd` - Text output complete
//! - `AgentEnd` - Execution finished
//! - Tool call events (Start, Delta, End, etc.)
//!
//! # Example
//!
//! ```no_run
//! use pipeline::{AgentExecutor, PiAgentClient, AgentClientConfig};
//! use pipeline::agent::{ProgressCallback, PiJsonEvent};
//!
//! struct LivePrinter;
//!
//! impl ProgressCallback for LivePrinter {
//!     fn on_event(&self, event: &PiJsonEvent) {
//!         match event {
//!             PiJsonEvent::TextDelta { delta } => print!("{}", delta),
//!             PiJsonEvent::AgentEnd => println!("\n[Complete]"),
//!             _ => {}
//!         }
//!     }
//! }
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let client = PiAgentClient::new(AgentClientConfig::new());
//! let callback = LivePrinter;
//!
//! // Stream execution with live output
//! let response = client.execute_streaming("Say hello", Some(&callback)).await?;
//!
//! println!("Final: {}", response.content);
//! # Ok(())
//! # }
//! ```

use crate::agent::PiJsonEvent;

/// Callback for processing events as they arrive from streaming execution
///
/// This trait is object-safe and can be used as `&dyn ProgressCallback`.
///
/// # Example
/// ```
/// # use pipeline::agent::{ProgressCallback, PiJsonEvent};
/// #
/// struct MyCallback {
///     events: Vec<PiJsonEvent>,
/// }
///
/// impl ProgressCallback for MyCallback {
///     fn on_event(&self, event: &PiJsonEvent) {
///         // Process each event
///         println!("{:?}", event);
///     }
/// }
/// ```
pub trait ProgressCallback: Send + Sync {
    /// Called for each parsed event during streaming execution
    ///
    /// # Arguments
    /// * `event` - The JSON event that was parsed from stdout
    ///
    /// This is called for every valid JSON line parsed from the subprocess,
    /// including events like `AgentStart`, `TextDelta`, `TextEnd`, `AgentEnd`, etc.
    fn on_event(&self, event: &PiJsonEvent);
}

/// No-op callback that does nothing (for backward compatibility)
#[derive(Debug, Clone, Default)]
pub struct NoopCallback;

impl ProgressCallback for NoopCallback {
    fn on_event(&self, _event: &PiJsonEvent) {
        // Do nothing
    }
}

/// Boxed callback for dynamic dispatch
pub type BoxedCallback = Box<dyn ProgressCallback>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[derive(Clone)]
    struct TestCallback {
        events: Arc<Mutex<Vec<PiJsonEvent>>>,
    }

    impl TestCallback {
        fn new() -> Self {
            Self {
                events: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn get_events(&self) -> Vec<PiJsonEvent> {
            self.events.lock().unwrap().clone()
        }
    }

    impl ProgressCallback for TestCallback {
        fn on_event(&self, event: &PiJsonEvent) {
            self.events.lock().unwrap().push(event.clone());
        }
    }

    #[test]
    fn test_noop_callback_does_nothing() {
        let callback = NoopCallback;
        callback.on_event(&PiJsonEvent::AgentStart);
        callback.on_event(&PiJsonEvent::TextDelta { delta: "test".to_string() });
        // Should not panic or crash
    }

    #[test]
    fn test_test_callback_collects_events() {
        let callback = TestCallback::new();

        callback.on_event(&PiJsonEvent::AgentStart);
        callback.on_event(&PiJsonEvent::TextDelta { delta: "Hello".to_string() });
        callback.on_event(&PiJsonEvent::AgentEnd);

        let events = callback.get_events();
        assert_eq!(events.len(), 3);
        assert_eq!(events[0], PiJsonEvent::AgentStart);
        assert_eq!(events[1], PiJsonEvent::TextDelta { delta: "Hello".to_string() });
        assert_eq!(events[2], PiJsonEvent::AgentEnd);
    }

    #[test]
    fn test_progress_callback_is_object_safe() {
        // This test verifies the trait is object-safe (can be used as dyn ProgressCallback)
        fn takes_callback(callback: &dyn ProgressCallback) {
            callback.on_event(&PiJsonEvent::AgentStart);
        }

        let noop = NoopCallback;
        takes_callback(&noop); // Should compile

        let test = TestCallback::new();
        takes_callback(&test); // Should compile
    }

    #[test]
    fn test_progress_callback_as_option() {
        // Test that callback can be passed as Option<&dyn ProgressCallback>
        fn with_optional_callback(callback: Option<&dyn ProgressCallback>) {
            if let Some(cb) = callback {
                cb.on_event(&PiJsonEvent::AgentStart);
            }
        }

        let noop = NoopCallback;
        with_optional_callback(Some(&noop)); // Should compile
        with_optional_callback(None); // Should compile
    }
}
