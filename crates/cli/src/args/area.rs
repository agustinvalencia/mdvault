use clap::{Args, Subcommand};

/// Area management subcommands.
#[derive(Debug, Subcommand)]
pub enum AreaCommands {
    /// Show area health report against defined standards
    Report(AreaReportArgs),

    /// Export area metrics as CSV or JSON
    Export(AreaExportArgs),
}

#[derive(Debug, Args)]
pub struct AreaReportArgs {
    /// Area name or ID
    pub area: String,

    /// Period: 'week', 'month', or specific like '2026-W11', '2026-03'
    #[arg(long, default_value = "week")]
    pub period: String,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct AreaExportArgs {
    /// Area name or ID
    pub area: String,

    /// Start date (YYYY-MM-DD)
    #[arg(long)]
    pub from: Option<String>,

    /// End date (YYYY-MM-DD)
    #[arg(long)]
    pub to: Option<String>,

    /// Output format (csv or json)
    #[arg(long, default_value = "csv")]
    pub format: String,
}
