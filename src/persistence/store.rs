//! SQLite-based persistence store

use crate::persistence::{PersistenceBackend, ExecutionSummary};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc, NaiveDateTime};
use sqlx::{SqlitePool, Row};
use uuid::Uuid;

/// SQLite execution store
pub struct SqliteExecutionStore {
    pool: SqlitePool,
}

impl SqliteExecutionStore {
    /// Create a new SQLite store
    pub async fn new(db_path: &str) -> Result<Self> {
        let pool = SqlitePool::connect(&format!("sqlite:{}", db_path))
            .await
            .context("Failed to connect to database")?;

        let store = Self { pool };
        store.init().await?;

        Ok(store)
    }

    /// Create store with default path
    pub async fn with_default_path() -> Result<Self> {
        let data_dir = dirs::data_local_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        let db_dir = data_dir.join("pipeline");
        std::fs::create_dir_all(&db_dir)?;

        let db_path = db_dir.join("executions.db");
        Self::new(db_path.to_str().unwrap()).await
    }

    /// Initialize database schema
    async fn init(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS executions (
                id TEXT PRIMARY KEY,
                pipeline_name TEXT NOT NULL,
                status TEXT NOT NULL,
                started_at TEXT NOT NULL,
                completed_at TEXT,
                progress REAL NOT NULL DEFAULT 0.0,
                completed_steps INTEGER NOT NULL DEFAULT 0,
                total_steps INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE INDEX IF NOT EXISTS idx_pipeline_name ON executions(pipeline_name);
            CREATE INDEX IF NOT EXISTS idx_status ON executions(status);
            CREATE INDEX IF NOT EXISTS idx_started_at ON executions(started_at);
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Convert DateTime<Utc> to NaiveDateTime for SQLite
    fn to_naive(dt: DateTime<Utc>) -> NaiveDateTime {
        dt.naive_utc()
    }

    /// Convert NaiveDateTime to DateTime<Utc>
    fn from_naive(dt: NaiveDateTime) -> DateTime<Utc> {
        DateTime::from_naive_utc_and_offset(dt, Utc)
    }
}

#[async_trait::async_trait]
impl PersistenceBackend for SqliteExecutionStore {
    async fn save_execution(&self, execution: &ExecutionSummary) -> Result<()> {
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO executions
            (id, pipeline_name, status, started_at, completed_at, progress, completed_steps, total_steps)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
        )
        .bind(execution.execution_id.to_string())
        .bind(&execution.pipeline_name)
       .bind(format!("{:?}", execution.status))
        .bind(Self::to_naive(execution.started_at))
        .bind(execution.completed_at.map(Self::to_naive))
        .bind(execution.progress)
        .bind(execution.completed_steps as i64)
        .bind(execution.total_steps as i64)
        .execute(&self.pool)
        .await
        .context("Failed to save execution")?;

        Ok(())
    }

    async fn load_execution(&self, execution_id: Uuid) -> Result<Option<ExecutionSummary>> {
        let row = sqlx::query(
            r#"
            SELECT id, pipeline_name, status, started_at, completed_at, progress, completed_steps, total_steps
            FROM executions
            WHERE id = ?1
            "#,
        )
        .bind(execution_id.to_string())
        .fetch_optional(&self.pool)
        .await
        .context("Failed to load execution")?;

        if let Some(row) = row {
            Ok(Some(ExecutionSummary {
                execution_id: Uuid::parse_str(&row.get::<String, _>("id"))?,
                pipeline_name: row.get("pipeline_name"),
                status: match row.get::<String, _>("status").as_str() {
                    "Pending" => crate::core::ExecutionStatus::Pending,
                    "Running" => crate::core::ExecutionStatus::Running,
                    "Completed" => crate::core::ExecutionStatus::Completed,
                    "Failed" => crate::core::ExecutionStatus::Failed,
                    "Cancelled" => crate::core::ExecutionStatus::Cancelled,
                    "Paused" => crate::core::ExecutionStatus::Paused,
                    _ => crate::core::ExecutionStatus::Pending,
                },
                started_at: Self::from_naive(row.get("started_at")),
                completed_at: row.get::<Option<NaiveDateTime>, _>("completed_at").map(Self::from_naive),
                progress: row.get("progress"),
                completed_steps: row.get::<i64, _>("completed_steps") as usize,
                total_steps: row.get::<i64, _>("total_steps") as usize,
            }))
        } else {
            Ok(None)
        }
    }

    async fn list_executions(
        &self,
        pipeline_name: &str,
    ) -> Result<Vec<ExecutionSummary>> {
        let rows = sqlx::query(
            r#"
            SELECT id, pipeline_name, status, started_at, completed_at, progress, completed_steps, total_steps
            FROM executions
            WHERE pipeline_name = ?1
            ORDER BY started_at DESC
            "#,
        )
        .bind(pipeline_name)
        .fetch_all(&self.pool)
        .await
        .context("Failed to list executions")?;

        rows.iter()
            .map(|row| {
                Ok(ExecutionSummary {
                    execution_id: Uuid::parse_str(&row.get::<String, _>("id"))?,
                    pipeline_name: row.get("pipeline_name"),
                    status: match row.get::<String, _>("status").as_str() {
                        "Pending" => crate::core::ExecutionStatus::Pending,
                        "Running" => crate::core::ExecutionStatus::Running,
                        "Completed" => crate::core::ExecutionStatus::Completed,
                        "Failed" => crate::core::ExecutionStatus::Failed,
                        "Cancelled" => crate::core::ExecutionStatus::Cancelled,
                        "Paused" => crate::core::ExecutionStatus::Paused,
                        _ => crate::core::ExecutionStatus::Pending,
                    },
                    started_at: Self::from_naive(row.get("started_at")),
                    completed_at: row.get::<Option<NaiveDateTime>, _>("completed_at").map(Self::from_naive),
                    progress: row.get("progress"),
                    completed_steps: row.get::<i64, _>("completed_steps") as usize,
                    total_steps: row.get::<i64, _>("total_steps") as usize,
                })
            })
            .collect()
    }

    async fn get_latest_execution(
        &self,
        pipeline_name: &str,
    ) -> Result<Option<ExecutionSummary>> {
        let row = sqlx::query(
            r#"
            SELECT id, pipeline_name, status, started_at, completed_at, progress, completed_steps, total_steps
            FROM executions
            WHERE pipeline_name = ?1
            ORDER BY started_at DESC
            LIMIT 1
            "#,
        )
        .bind(pipeline_name)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get latest execution")?;

        if let Some(row) = row {
            Ok(Some(ExecutionSummary {
                execution_id: Uuid::parse_str(&row.get::<String, _>("id"))?,
                pipeline_name: row.get("pipeline_name"),
                status: match row.get::<String, _>("status").as_str() {
                    "Pending" => crate::core::ExecutionStatus::Pending,
                    "Running" => crate::core::ExecutionStatus::Running,
                    "Completed" => crate::core::ExecutionStatus::Completed,
                    "Failed" => crate::core::ExecutionStatus::Failed,
                    "Cancelled" => crate::core::ExecutionStatus::Cancelled,
                    "Paused" => crate::core::ExecutionStatus::Paused,
                    _ => crate::core::ExecutionStatus::Pending,
                },
                started_at: Self::from_naive(row.get("started_at")),
                completed_at: row.get::<Option<NaiveDateTime>, _>("completed_at").map(Self::from_naive),
                progress: row.get("progress"),
                completed_steps: row.get::<i64, _>("completed_steps") as usize,
                total_steps: row.get::<i64, _>("total_steps") as usize,
            }))
        } else {
            Ok(None)
        }
    }

    async fn delete_execution(&self, execution_id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM executions WHERE id = ?1")
            .bind(execution_id.to_string())
            .execute(&self.pool)
            .await
            .context("Failed to delete execution")?;

        Ok(())
    }

    async fn list_pipelines(&self) -> Result<Vec<String>> {
        let rows = sqlx::query(
            r#"
            SELECT DISTINCT pipeline_name
            FROM executions
            ORDER BY pipeline_name ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to list pipelines")?;

        Ok(rows.iter().map(|row| row.get("pipeline_name")).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ExecutionStatus;

    #[tokio::test]
    async fn test_sqlite_store() {
        let store = SqliteExecutionStore::new(":memory:").await.unwrap();

        let summary = ExecutionSummary {
            execution_id: Uuid::new_v4(),
            pipeline_name: "test-pipeline".to_string(),
            status: ExecutionStatus::Completed,
            started_at: Utc::now(),
            completed_at: Some(Utc::now()),
            progress: 1.0,
            completed_steps: 3,
            total_steps: 3,
        };

        store.save_execution(&summary).await.unwrap();

        let loaded = store
            .load_execution(summary.execution_id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(loaded.pipeline_name, summary.pipeline_name);
        assert_eq!(loaded.status, summary.status);
    }
}
