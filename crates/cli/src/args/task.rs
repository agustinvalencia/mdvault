use clap::{Args, Subcommand};
use clap_complete::engine::ArgValueCompleter;
use std::path::PathBuf;

use super::StatusFilter;

/// Task management subcommands.
#[derive(Debug, Subcommand)]
pub enum TaskCommands {
    /// List tasks with optional filters
    List(TaskListArgs),

    /// Mark a task as done
    Done(TaskDoneArgs),

    /// Cancel a task
    Cancel(TaskCancelArgs),

    /// Show detailed status for a task
    Status(TaskStatusArgs),
}

#[derive(Debug, Args)]
pub struct TaskListArgs {
    /// Filter by project name
    #[arg(long, short)]
    pub project: Option<String>,

    /// Filter by status (todo, in-progress, done, blocked, cancelled)
    #[arg(long, short, value_enum)]
    pub status: Option<StatusFilter>,
}

#[derive(Debug, Args)]
pub struct TaskDoneArgs {
    /// Path to the task note (relative to vault root)
    #[arg(add = ArgValueCompleter::new(crate::completions::complete_notes))]
    pub task: PathBuf,

    /// Summary of what was done (logged to task)
    #[arg(long, short)]
    pub summary: Option<String>,
}

#[derive(Debug, Args)]
pub struct TaskCancelArgs {
    /// Path to the task note (relative to vault root)
    #[arg(add = ArgValueCompleter::new(crate::completions::complete_notes))]
    pub task: PathBuf,

    /// Reason for cancellation (logged to task)
    #[arg(long, short)]
    pub reason: Option<String>,
}

#[derive(Debug, Args)]
pub struct TaskStatusArgs {
    /// Task ID (e.g., "MCP-001")
    #[arg(add = ArgValueCompleter::new(crate::completions::complete_notes))]
    pub task_id: String,
}
