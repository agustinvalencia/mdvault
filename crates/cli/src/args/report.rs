use clap::{Args, Subcommand};
use clap_complete::engine::ArgValueCompleter;

#[derive(Debug, Args)]
pub struct ReportArgs {
    /// Generate report for a specific month (YYYY-MM format)
    #[arg(long, conflicts_with_all = ["week", "dashboard"])]
    pub month: Option<String>,

    /// Generate report for a specific week (YYYY-WXX format)
    #[arg(long, conflicts_with_all = ["month", "dashboard"])]
    pub week: Option<String>,

    /// Output report to a markdown file instead of terminal
    #[arg(long, short)]
    pub output: Option<std::path::PathBuf>,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,

    /// Generate a dashboard report (project-aware, for TUI/charts/MCP)
    #[arg(long, short, conflicts_with_all = ["month", "week"])]
    pub dashboard: bool,

    /// Scope to a specific project (ID or folder name). Requires --dashboard or --visual.
    #[arg(long, short, add = ArgValueCompleter::new(crate::completions::complete_projects))]
    pub project: Option<String>,

    /// Days of activity history to include in dashboard (default: 30)
    #[arg(long, default_value = "30")]
    pub activity_days: u32,

    /// Generate a visual PNG dashboard (implies --dashboard)
    #[arg(long, short, conflicts_with_all = ["month", "week"])]
    pub visual: bool,
}

/// Today command subcommands.
#[derive(Debug, Subcommand)]
pub enum TodayCommands {
    /// Open today's daily note in default editor
    Open,
}

#[derive(Debug, Args)]
#[command(after_help = "\
Examples:
  mdv today                         # Smart dashboard (auto-selects plan/review based on time)
  mdv today --plan                  # Force morning planning mode
  mdv today --review                # Force evening review mode
  mdv today open                    # Open today's daily note in $EDITOR
")]
pub struct TodayArgs {
    /// Force morning planning mode
    #[arg(long, conflicts_with = "review")]
    pub plan: bool,

    /// Force evening review mode
    #[arg(long, conflicts_with = "plan")]
    pub review: bool,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,

    #[command(subcommand)]
    pub command: Option<TodayCommands>,
}
