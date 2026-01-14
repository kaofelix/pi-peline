//! Persistence layer for pipeline execution history

#[cfg(feature = "sqlite")]
pub mod store;

#[cfg(feature = "sqlite")]
pub use store::SqliteExecutionStore;

pub use crate::core::ExecutionStatus;
use crate::core::{Pipeline};
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Summary of a pipeline execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionSummary {
    /// Unique execution ID
    pub execution_id: Uuid,

    /// Pipeline name
    pub pipeline_name: String,

    /// Execution status
    pub status: ExecutionStatus,

    /// When execution started
    pub started_at: DateTime<Utc>,

    /// When execution completed (if complete)
    pub completed_at: Option<DateTime<Utc>>,

    /// Progress (0.0 to 1.0)
    pub progress: f64,

    /// Number of completed steps
    pub completed_steps: usize,

    /// Total number of steps
    pub total_steps: usize,
}

/// Trait for persistence backends
#[async_trait::async_trait]
pub trait PersistenceBackend: Send + Sync {
    /// Save a pipeline execution
    async fn save_execution(&self, execution: &ExecutionSummary) -> Result<()>;

    /// Load an execution by ID
    async fn load_execution(&self, execution_id: Uuid) -> Result<Option<ExecutionSummary>>;

    /// List all executions for a pipeline
    async fn list_executions(
        &self,
        pipeline_name: &str,
    ) -> Result<Vec<ExecutionSummary>>;

    /// List all pipeline names
    async fn list_pipelines(&self) -> Result<Vec<String>>;
}

/// In-memory persistence (for testing or ephemeral use)
pub struct InMemoryPersistence {
    executions: tokio::sync::RwLock<std::collections::HashMap<Uuid, ExecutionSummary>>,
    by_pipeline: tokio::sync::RwLock<std::collections::HashMap<String, Vec<Uuid>>>,
}

impl InMemoryPersistence {
    pub fn new() -> Self {
        Self {
            executions: tokio::sync::RwLock::new(std::collections::HashMap::new()),
            by_pipeline: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        }
    }
}

impl Default for InMemoryPersistence {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl PersistenceBackend for InMemoryPersistence {
    async fn save_execution(&self, execution: &ExecutionSummary) -> Result<()> {
        let mut execs = self.executions.write().await;
        execs.insert(execution.execution_id, execution.clone());

        let mut by_pipeline = self.by_pipeline.write().await;
        by_pipeline
            .entry(execution.pipeline_name.clone())
            .or_insert_with(Vec::new)
            .push(execution.execution_id);

        Ok(())
    }

    async fn load_execution(&self, execution_id: Uuid) -> Result<Option<ExecutionSummary>> {
        let execs = self.executions.read().await;
        Ok(execs.get(&execution_id).cloned())
    }

    async fn list_executions(
        &self,
        pipeline_name: &str,
    ) -> Result<Vec<ExecutionSummary>> {
        let execs = self.executions.read().await;
        let by_pipeline = self.by_pipeline.read().await;

        if let Some(ids) = by_pipeline.get(pipeline_name) {
            let mut result = Vec::new();
            for id in ids {
                if let Some(exec) = execs.get(id) {
                    result.push(exec.clone());
                }
            }
            Ok(result)
        } else {
            Ok(Vec::new())
        }
    }

    async fn list_pipelines(&self) -> Result<Vec<String>> {
        let by_pipeline = self.by_pipeline.read().await;
        Ok(by_pipeline.keys().cloned().collect())
    }
}

/// Create a summary from a pipeline
pub fn create_summary(pipeline: &Pipeline) -> ExecutionSummary {
    ExecutionSummary {
        execution_id: pipeline.state.execution_id,
        pipeline_name: pipeline.name.clone(),
        status: pipeline.state.status,
        started_at: pipeline.state.started_at.unwrap_or_else(Utc::now),
        completed_at: pipeline.state.completed_at,
        progress: pipeline.state.progress(),
        completed_steps: pipeline.state.completed_steps,
        total_steps: pipeline.state.total_steps,
    }
}
