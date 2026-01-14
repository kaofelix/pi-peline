mod agent;
mod cli;
mod core;
mod execution;
mod persistence;

use cli::output::{INFO, style};

use anyhow::{Context, Result};
use cli::{Cli, Command};
use cli::commands::{RunCommand, ValidateCommand, ListCommand, HistoryCommand, SchedulingStrategyArg};
use cli::output::*;
use execution::{ExecutionEngine, SchedulingStrategy, ExecutionEvent};
use agent::{PiAgentClient, AgentClientConfig};
use persistence::{SqliteExecutionStore, InMemoryPersistence, PersistenceBackend, create_summary, ExecutionSummary};
use std::sync::Arc;
use tracing::{error, Level};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::from_args();

    // Initialize logging
    let log_level = if cli.verbose { Level::DEBUG } else { Level::INFO };
    let subscriber = FmtSubscriber::builder()
        .with_max_level(log_level)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .context("Failed to set logging subscriber")?;

    // Execute command
    match &cli.command {
        Command::Run(cmd) => run_pipeline(cmd, cli.clone()).await?,
        Command::Validate(cmd) => validate_pipeline(cmd)?,
        Command::List(cmd) => list_pipelines(cmd).await?,
        Command::History(cmd) => show_history(cmd).await?,
    }

    Ok(())
}

async fn run_pipeline(cmd: &RunCommand, cli: Cli) -> Result<()> {
    // Load pipeline config
    let config = core::config::PipelineConfig::from_file(&cmd.file)
        .context("Failed to load pipeline config")?;

    println!(
        "{} Loaded pipeline: {}",
        INFO,
        style(&config.name).bold()
    );

    // Create pipeline
    let mut pipeline = config.to_pipeline();

    // Apply variable overrides
    for (key, value) in &cmd.variable {
        pipeline.variables.insert(key.clone(), value.clone());
        println!(
            "{} Variable override: {} = {}",
            INFO,
            style(key).cyan(),
            style(value).dim()
        );
    }

    // Set up persistence
    let store: Arc<dyn PersistenceBackend> = if cmd.no_history {
        Arc::new(InMemoryPersistence::new())
    } else {
        Arc::new(SqliteExecutionStore::with_default_path().await?)
    };

    // Create agent client (mock for now - TODO: implement actual Pi client)
    let agent_config = AgentClientConfig::default();
    let agent = PiAgentClient::new(agent_config);

    // Convert scheduling strategy
    let strategy: SchedulingStrategy = match cmd.strategy {
        SchedulingStrategyArg::Sequential => SchedulingStrategy::Sequential,
        SchedulingStrategyArg::Parallel => SchedulingStrategy::Parallel,
        SchedulingStrategyArg::ParallelLimited => SchedulingStrategy::LimitedParallel(4),
    };

    // Create execution engine
    let engine = ExecutionEngine::new(agent, strategy);

    // Set up event handler for console output
    let stream = cli.stream;
    engine.add_event_handler(move |event| {
        println!("{}", format_execution_event(&event));

        // For streaming, show step output as it arrives
        if stream {
            if let ExecutionEvent::StepOutput { output, .. } = &event {
                println!("{}", format_output(output, 5));
            }
        }
    });

    // Execute pipeline
    println!();
    let result = engine.execute(&mut pipeline).await;

    // Save to history
    if !cmd.no_history {
        let summary = create_summary(&pipeline);
        store.save_execution(&summary).await?;
        println!(
            "\n{} Execution saved to history (ID: {})",
            INFO,
            style(&summary.execution_id.to_string()[..8]).dim()
        );
    }

    // Print final status
    if result.is_ok() {
        println!(
            "\n{} {} completed {}",
            CHECK,
            style(&pipeline.name).bold(),
            style("successfully").green()
        );
    } else {
        println!(
            "\n{} {} {}",
            CROSS,
            style(&pipeline.name).bold(),
            style("failed").red()
        );
        error!("{}", result.unwrap_err());
        std::process::exit(1);
    }

    Ok(())
}

fn validate_pipeline(cmd: &ValidateCommand) -> Result<()> {
    println!("{} Validating pipeline...", INFO);

    let result = core::config::PipelineConfig::from_file(&cmd.file);

    match result {
        Ok(config) => {
            println!("{} Pipeline configuration is valid!", CHECK);
            println!("  Name: {}", style(&config.name).bold());
            println!("  Steps: {}", style(config.steps.len()).cyan());
            println!("  Variables: {}", style(config.variables.len()).cyan());

            if cmd.json {
                let json = serde_json::to_string_pretty(&config)?;
                println!("\n{}", json);
            }
            Ok(())
        }
        Err(e) => {
            println!("{} Validation failed:", CROSS);
            println!("  {}", style(e).red());
            std::process::exit(1);
        }
    }
}

async fn list_pipelines(cmd: &ListCommand) -> Result<()> {
    let store = SqliteExecutionStore::with_default_path().await?;
    let pipelines = store.list_pipelines().await?;

    if pipelines.is_empty() {
        println!("{} No pipelines found in history", INFO);
        return Ok(());
    }

    println!("{} Pipelines in history:", INFO);

    for pipeline_name in &pipelines {
        let executions = store.list_executions(pipeline_name).await?;

        if cmd.with_counts {
            let completed = executions.iter().filter(|e| e.status == ExecutionStatus::Completed).count();
            let failed = executions.iter().filter(|e| e.status == ExecutionStatus::Failed).count();
            println!(
                "  {} ({} runs: {} succeeded, {} failed)",
                style(pipeline_name).bold(),
                style(executions.len()).cyan(),
                style(completed).green(),
                style(failed).red()
            );
        } else {
            println!("  {}", style(pipeline_name).bold());
        }
    }

    if cmd.json {
        let mut json_data = Vec::new();
        for pipeline in &pipelines {
            let executions = store.list_executions(pipeline).await.ok();
            json_data.push(serde_json::json!({
                "name": pipeline,
                "execution_count": executions.as_ref().map(|e| e.len()).unwrap_or(0)
            }));
        }
        let data = serde_json::json!({ "pipelines": json_data });
        println!("\n{}", serde_json::to_string_pretty(&data)?);
    }

    Ok(())
}

async fn show_history(cmd: &HistoryCommand) -> Result<()> {
    let store = SqliteExecutionStore::with_default_path().await?;

    // If specific execution ID is requested
    if let Some(exec_id_str) = &cmd.execution_id {
        let exec_id = uuid::Uuid::parse_str(exec_id_str)
            .context("Invalid execution ID format")?;
        let summary = store.load_execution(exec_id).await?;

        match summary {
            Some(summary) => {
                print_execution_details(&summary, cmd.verbose)?;
            }
            None => {
                println!("{} Execution not found", WARN);
            }
        }
        return Ok(());
    }

    // List executions for pipeline or all
    let executions = if let Some(pipeline_name) = &cmd.pipeline {
        store.list_executions(pipeline_name).await?
    } else {
        // Get all executions across all pipelines
        let pipelines = store.list_pipelines().await?;
        let mut all_execs = Vec::new();
        for pipeline in &pipelines {
            all_execs.extend(store.list_executions(pipeline).await?);
        }
        // Sort by started_at descending
        all_execs.sort_by(|a, b| b.started_at.cmp(&a.started_at));
        all_execs.into_iter().take(cmd.limit).collect()
    };

    if executions.is_empty() {
        println!("{} No executions found", INFO);
        return Ok(());
    }

    println!("{} Execution history (showing latest {}):", INFO, cmd.limit);

    if cmd.json {
        let data = serde_json::json!({ "executions": executions });
        println!("{}", serde_json::to_string_pretty(&data)?);
    } else {
        for summary in &executions {
            println!("  {}", format_execution_summary(summary));
        }
    }

    Ok(())
}

fn print_execution_details(summary: &ExecutionSummary, verbose: bool) -> Result<()> {
    println!("{} Execution Details", INFO);
    println!("  ID: {}", style(summary.execution_id).cyan());
    println!("  Pipeline: {}", style(&summary.pipeline_name).bold());
    println!("  Status: {}", format_status(summary.status));
    println!("  Started: {}", style(summary.started_at.to_rfc3339()).dim());
    if let Some(completed) = summary.completed_at {
        println!(
            "  Completed: {}",
            style(completed.to_rfc3339()).dim()
        );
        if let Ok(duration) = completed.signed_duration_since(summary.started_at).to_std() {
            println!("  Duration: {}", style(format_duration(duration)).dim());
        }
    }
    println!("  Progress: {} ({}/{})",
        style(format!("{:.0}%", summary.progress * 100.0)).cyan(),
        summary.completed_steps,
        summary.total_steps
    );

    if verbose {
        println!("\n  {}", style("Full details:").bold());
        let json = serde_json::to_string_pretty(summary)?;
        for line in json.lines() {
            println!("    {}", line);
        }
    }

    Ok(())
}

fn format_duration(duration: std::time::Duration) -> String {
    let secs = duration.as_secs();
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{}h {}m {}s", secs / 3600, (secs % 3600) / 60, secs % 60)
    }
}

use core::ExecutionStatus;
