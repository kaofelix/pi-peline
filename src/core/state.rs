//! Execution state models

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Overall pipeline execution status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionStatus {
    /// Pipeline has not started
    Pending,
    /// Pipeline is currently running
    Running,
    /// Pipeline completed successfully
    Completed,
    /// Pipeline failed
    Failed,
    /// Pipeline was cancelled
    Cancelled,
    /// Pipeline is paused (for future distributed execution)
    Paused,
}

/// State of a single step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StepState {
    /// Step is waiting for dependencies (first execution)
    Pending,
    /// Step is waiting to be retried (preserves attempt count)
    Retrying {
        attempt: usize,
    },
    /// Step is currently running
    Running {
        started_at: DateTime<Utc>,
        attempt: usize,
    },
    /// Step completed successfully
    Completed {
        output: String,
        attempts: usize,
        started_at: DateTime<Utc>,
        completed_at: DateTime<Utc>,
    },
    /// Step failed (all retries exhausted)
    Failed {
        error: String,
        attempts: usize,
        last_started_at: DateTime<Utc>,
        failed_at: DateTime<Utc>,
    },
    /// Step was skipped (e.g., conditional execution)
    Skipped {
        reason: String,
    },
    /// Step is blocked (waiting on external condition)
    Blocked {
        reason: String,
        blocked_at: DateTime<Utc>,
    },
}

impl StepState {
    /// Check if step is in a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            StepState::Completed { .. } | StepState::Failed { .. } | StepState::Skipped { .. }
        )
    }
}

/// Overall pipeline state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineState {
    /// Unique execution ID
    pub execution_id: Uuid,

    /// Current execution status
    pub status: ExecutionStatus,

    /// When execution started
    pub started_at: Option<DateTime<Utc>>,

    /// When execution completed/failed
    pub completed_at: Option<DateTime<Utc>>,

    /// Total number of steps
    pub total_steps: usize,

    /// Number of completed steps
    pub completed_steps: usize,

    /// Number of failed steps
    pub failed_steps: usize,

    /// Number of currently running steps
    pub running_steps: usize,
}

impl PipelineState {
    /// Create a new pipeline state
    pub fn new() -> Self {
        Self {
            execution_id: Uuid::new_v4(),
            status: ExecutionStatus::Pending,
            started_at: None,
            completed_at: None,
            total_steps: 0,
            completed_steps: 0,
            failed_steps: 0,
            running_steps: 0,
        }
    }

    /// Mark pipeline as started
    pub fn start(&mut self, total_steps: usize) {
        self.status = ExecutionStatus::Running;
        self.started_at = Some(Utc::now());
        self.total_steps = total_steps;
    }

    /// Mark pipeline as completed
    pub fn complete(&mut self) {
        self.status = ExecutionStatus::Completed;
        self.completed_at = Some(Utc::now());
    }

    /// Mark pipeline as failed
    pub fn fail(&mut self) {
        self.status = ExecutionStatus::Failed;
        self.completed_at = Some(Utc::now());
    }

    /// Update step counts based on current steps
    pub fn update_counts(&mut self, steps: &usize, completed: &usize, failed: &usize, running: &usize) {
        self.total_steps = *steps;
        self.completed_steps = *completed;
        self.failed_steps = *failed;
        self.running_steps = *running;
    }

    /// Calculate progress percentage (0.0 to 1.0)
    pub fn progress(&self) -> f64 {
        if self.total_steps == 0 {
            return 0.0;
        }
        (self.completed_steps + self.failed_steps) as f64 / self.total_steps as f64
    }
}

impl Default for PipelineState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_step_state_is_terminal() {
        assert!(StepState::Pending.is_terminal() == false);
        assert!(StepState::Running {
            started_at: Utc::now(),
            attempt: 1
        }
        .is_terminal() == false);
        assert!(StepState::Completed {
            output: "test".to_string(),
            attempts: 1,
            started_at: Utc::now(),
            completed_at: Utc::now()
        }
        .is_terminal());
        assert!(StepState::Failed {
            error: "test".to_string(),
            attempts: 1,
            last_started_at: Utc::now(),
            failed_at: Utc::now()
        }
        .is_terminal());
        assert!(StepState::Skipped {
            reason: "test".to_string()
        }
        .is_terminal());
    }

    #[test]
    fn test_pipeline_progress() {
        let mut state = PipelineState::new();
        state.start(10);
        assert_eq!(state.progress(), 0.0);

        state.completed_steps = 5;
        assert_eq!(state.progress(), 0.5);

        state.completed_steps = 10;
        assert_eq!(state.progress(), 1.0);
    }
}
