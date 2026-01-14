//! Command-line interface

pub mod commands;
pub mod output;

use clap::{Parser, Subcommand};
use commands::{RunCommand, ValidateCommand, ListCommand, HistoryCommand};

/// Pipeline CI/CD tool powered by Pi agent
#[derive(Debug, Parser, Clone)]
#[command(name = "pipeline")]
#[command(author = "Pipeline Contributors")]
#[command(version = "0.1.0")]
#[command(about = "A CI/CD pipeline tool powered by Pi agent", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Path to pipeline configuration file
    #[arg(short, long, global = true)]
    pub config: Option<String>,

    /// Enable streaming output
    #[arg(short, long, global = true)]
    pub stream: bool,
}

/// Available commands
#[derive(Debug, Subcommand, Clone)]
pub enum Command {
    /// Run a pipeline
    Run(RunCommand),

    /// Validate a pipeline configuration
    Validate(ValidateCommand),

    /// List available pipelines
    List(ListCommand),

    /// Show execution history
    History(HistoryCommand),
}

impl Cli {
    /// Parse CLI arguments from environment
    pub fn from_args() -> Self {
        Self::parse()
    }

    /// Parse CLI arguments from a slice
    pub fn try_parse_from<I, T>(itr: I) -> Result<Self, clap::Error>
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString> + Clone,
    {
        <Self as Parser>::try_parse_from(itr)
    }
}

use std::ffi::OsString;
