use clap::Args;
use clap_complete::engine::ArgValueCompleter;

#[derive(Debug, Args)]
#[command(after_help = "\
Examples:
  mdv dashboard                      # Vault-wide interactive dashboard
  mdv dashboard --project MCP        # Dashboard scoped to project MCP
  mdv dashboard --activity-days 60   # Include 60 days of activity
")]
pub struct DashboardArgs {
    /// Scope to a specific project (ID or folder name)
    #[arg(long, short, add = ArgValueCompleter::new(crate::completions::complete_projects))]
    pub project: Option<String>,

    /// Days of activity history to include (default: 30)
    #[arg(long, default_value = "30")]
    pub activity_days: u32,
}
