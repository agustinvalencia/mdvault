use clap::{Args, Subcommand};
use clap_complete::engine::ArgValueCompleter;

/// Project management subcommands.
#[derive(Debug, Subcommand)]
pub enum ProjectCommands {
    /// List all projects with task counts
    List(ProjectListArgs),

    /// Show project status with tasks in kanban-style view
    Status(ProjectStatusArgs),

    /// Show project progress with completion metrics and velocity
    Progress(ProjectProgressArgs),

    /// Archive a completed project
    Archive(ProjectArchiveArgs),
}

#[derive(Debug, Args)]
pub struct ProjectListArgs {
    /// Filter by status (active, completed, on-hold, archived)
    #[arg(long, short)]
    pub status: Option<String>,

    /// Filter by kind (project, area)
    #[arg(long, short)]
    pub kind: Option<String>,
}

#[derive(Debug, Args)]
pub struct ProjectStatusArgs {
    /// Project ID or folder name (e.g., "MCP" or "my-cool-project")
    #[arg(add = ArgValueCompleter::new(crate::completions::complete_projects))]
    pub project: String,
}

#[derive(Debug, Args)]
pub struct ProjectProgressArgs {
    /// Project ID or folder name (optional - shows all projects if omitted)
    #[arg(add = ArgValueCompleter::new(crate::completions::complete_projects))]
    pub project: Option<String>,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,

    /// Include archived projects
    #[arg(long)]
    pub include_archived: bool,
}

#[derive(Debug, Args)]
pub struct ProjectArchiveArgs {
    /// Project ID or folder name
    #[arg(add = ArgValueCompleter::new(crate::completions::complete_projects))]
    pub project: String,

    /// Skip confirmation prompts
    #[arg(long, short)]
    pub yes: bool,
}
