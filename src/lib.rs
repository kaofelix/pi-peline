//! pi-peline - A CI/CD pipeline tool powered by Pi agent

pub mod agent;
pub mod cli;
pub mod core;
pub mod execution;
pub mod persistence;

// Re-export commonly used types
pub use agent::{AgentExecutor, AgentResponse, AgentError, PiAgentClient, AgentClientConfig};
pub use agent::{PiJsonEvent, ProgressCallback};
pub use core::{Pipeline, Step, StepState, PipelineContext, ExecutionStatus};
pub use execution::{ExecutionEngine, SchedulingStrategy, ExecutionEvent};
