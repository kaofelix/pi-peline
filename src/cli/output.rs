//! CLI output formatting

use crate::{
    core::{ExecutionStatus},
    persistence::ExecutionSummary,
    execution::ContinueAction,
};
use console::Emoji;

// Re-export style
pub use console::style;

// Emojis for output
pub static CHECK: Emoji<'_, '_> = Emoji("✅ ", "✓ ");
pub static CROSS: Emoji<'_, '_> = Emoji("❌ ", "✗ ");
pub static SPINNER: Emoji<'_, '_> = Emoji("⏳ ", "~ ");
pub static INFO: Emoji<'_, '_> = Emoji("ℹ️  ", "i ");
pub static WARN: Emoji<'_, '_> = Emoji("⚠️  ", "!");
pub static ROCKET: Emoji<'_, '_> = Emoji("🚀 ", "> ");

/// Format an execution status for display
pub fn format_status(status: ExecutionStatus) -> String {
    match status {
        ExecutionStatus::Pending => style("PENDING").dim().to_string(),
        ExecutionStatus::Running => style("RUNNING").yellow().to_string(),
        ExecutionStatus::Completed => style("COMPLETED").green().to_string(),
        ExecutionStatus::Failed => style("FAILED").red().to_string(),
        ExecutionStatus::Cancelled => style("CANCELLED").yellow().to_string(),
        ExecutionStatus::Paused => style("PAUSED").blue().to_string(),
    }
}

/// Format execution summary for display
pub fn format_execution_summary(summary: &ExecutionSummary) -> String {
    let status_icon = match summary.status {
        ExecutionStatus::Completed => CHECK,
        ExecutionStatus::Failed => CROSS,
        ExecutionStatus::Running => SPINNER,
        _ => INFO,
    };

    format!(
        "{} {} - {} - {} ({}/{}) - {}",
        status_icon,
        style(&summary.execution_id.to_string()[..8]).dim(),
        style(&summary.pipeline_name).bold(),
        format_status(summary.status),
        summary.completed_steps,
        summary.total_steps,
        style(format!("{:.0}%", summary.progress * 100.0))
            .cyan()
    )
}

/// Format an execution event for display
pub fn format_execution_event(
    event: &crate::execution::ExecutionEvent,
) -> String {
    match event {
        crate::execution::ExecutionEvent::PipelineStarted {
            execution_id,
            pipeline_name,
        } => format!(
            "{} Starting pipeline {} ({})",
            ROCKET,
            style(pipeline_name).bold(),
            style(&execution_id.to_string()[..8]).dim()
        ),
        crate::execution::ExecutionEvent::StepStarted { step_id, attempt } => {
            if *attempt > 1 {
                format!(
                    "{} {} (retry {}/{})",
                    SPINNER,
                    style(step_id).cyan(),
                    style(attempt - 1).dim(),
                    style(3).dim()
                )
            } else {
                format!("{} {}", SPINNER, style(step_id).cyan())
            }
        }
        crate::execution::ExecutionEvent::StepOutput { step_id, output } => {
            format!("{} Output from {}:\n{}", INFO, style(step_id).dim(), output)
        }
        crate::execution::ExecutionEvent::StepCompleted {
            step_id,
            next_step,
        } => {
            if let Some(next) = next_step {
                format!(
                    "{} {} → {}",
                    CHECK,
                    style(step_id).green(),
                    style(next).cyan()
                )
            } else {
                format!("{} {}", CHECK, style(step_id).green())
            }
        }
        crate::execution::ExecutionEvent::StepFailed { step_id, error } => {
            format!("{} {}: {}", CROSS, style(step_id).red(), style(error).dim())
        }
        crate::execution::ExecutionEvent::StepContinued { step_id, action } => {
            let action_str = match action {
                ContinueAction::Retry => "retrying".to_string(),
                ContinueAction::Route(target) => format!("routing to {}", target),
            };
            format!("{} {} ({})", INFO, style(step_id).yellow(), action_str)
        }
        crate::execution::ExecutionEvent::StepRetrying {
            step_id,
            attempt,
            max_retries,
        } => format!(
            "{} {} (attempt {}/{})",
            WARN, style(step_id).yellow(), attempt, max_retries
        ),
        crate::execution::ExecutionEvent::StepRerouted {
            from_step,
            to_step,
        } => format!(
            "{} {} → {}",
            INFO,
            style(from_step).dim(),
            style(to_step).cyan()
        ),
        crate::execution::ExecutionEvent::PipelineCompleted {
            execution_id,
            status,
        } => {
            let status_str = match status {
                ExecutionStatus::Completed => format!("{} completed", style("successfully").green()),
                ExecutionStatus::Failed => style("failed").red().to_string(),
                _ => format!("{:?}", status),
            };
            format!(
                "{} Pipeline ({}) {}",
                INFO,
                style(&execution_id.to_string()[..8]).dim(),
                status_str
            )
        }
    }
}

/// Format step output with truncation
pub fn format_output(output: &str, max_lines: usize) -> String {
    let lines: Vec<&str> = output.lines().collect();

    if lines.len() <= max_lines {
        output.to_string()
    } else {
        let truncated = lines[..max_lines].join("\n");
        format!(
            "{}\n{}... ({} more lines)",
            truncated,
            style("[truncated]").dim(),
            lines.len() - max_lines
        )
    }
}
