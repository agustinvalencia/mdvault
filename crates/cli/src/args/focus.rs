use clap::Args;
use clap_complete::engine::ArgValueCompleter;

#[derive(Debug, Args)]
#[command(after_help = "\
Examples:
  mdv focus                           # Show current focus
  mdv focus MCP                       # Set focus to project MCP
  mdv focus MCP --note \"OAuth work\"   # Set focus with note
  mdv focus --clear                   # Clear focus
")]
pub struct FocusArgs {
    /// Project ID to focus on (e.g., "MCP", "VAULT")
    #[arg(add = ArgValueCompleter::new(crate::completions::complete_projects))]
    pub project: Option<String>,

    /// Note describing current work
    #[arg(long, short)]
    pub note: Option<String>,

    /// Clear the current focus
    #[arg(long, short)]
    pub clear: bool,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}
