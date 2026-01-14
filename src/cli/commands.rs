//! CLI command definitions

use clap::{Args, Subcommand};
use crate::execution::SchedulingStrategy;
use crate::persistence::ExecutionSummary;

/// Run a pipeline
#[derive(Debug, Args, Clone)]
pub struct RunCommand {
    /// Path to pipeline YAML file
    #[arg(short, long)]
    pub file: String,

    /// Variable overrides (key=value)
    #[arg(long, value_parser = parse_key_value)]
    pub variable: Vec<(String, String)>,

    /// Scheduling strategy
    #[arg(long, value_enum, default_value_t = SchedulingStrategyArg::Sequential)]
    pub strategy: SchedulingStrategyArg,

    /// Don't save execution to history
    #[arg(long)]
    pub no_history: bool,

    /// Specific step to start from (for debugging/resuming)
    #[arg(long)]
    pub from_step: Option<String>,
}

/// Validate a pipeline configuration
#[derive(Debug, Args, Clone)]
pub struct ValidateCommand {
    /// Path to pipeline YAML file
    #[arg(short, long)]
    pub file: String,

    /// Output in JSON format
    #[arg(long)]
    pub json: bool,
}

/// List available pipelines
#[derive(Debug, Args, Clone)]
pub struct ListCommand {
    /// Show execution counts
    #[arg(long)]
    pub with_counts: bool,

    /// Output in JSON format
    #[arg(long)]
    pub json: bool,
}

/// Show execution history
#[derive(Debug, Args, Clone)]
pub struct HistoryCommand {
    /// Pipeline name to filter by
    #[arg(short, long)]
    pub pipeline: Option<String>,

    /// Number of recent executions to show
    #[arg(short, long, default_value_t = 10)]
    pub limit: usize,

    /// Show full details
    #[arg(long)]
    pub verbose: bool,

    /// Output in JSON format
    #[arg(long)]
    pub json: bool,

    /// Show executions for a specific execution ID
    #[arg(long)]
    pub execution_id: Option<String>,
}

/// Scheduling strategy argument
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum SchedulingStrategyArg {
    Sequential,
    Parallel,
    #[clap(name = "parallel-limited")]
    ParallelLimited,
}

impl From<SchedulingStrategyArg> for SchedulingStrategy {
    fn from(arg: SchedulingStrategyArg) -> Self {
        match arg {
            SchedulingStrategyArg::Sequential => SchedulingStrategy::Sequential,
            SchedulingStrategyArg::Parallel => SchedulingStrategy::Parallel,
            SchedulingStrategyArg::ParallelLimited => SchedulingStrategy::LimitedParallel(4),
        }
    }
}

/// Parse key=value pairs
pub fn parse_key_value(s: &str) -> Result<(String, String), String> {
    let parts: Vec<&str> = s.splitn(2, '=').collect();
    if parts.len() != 2 {
        return Err(format!("Invalid key=value pair: {}", s));
    }
    Ok((parts[0].to_string(), parts[1].to_string()))
}
