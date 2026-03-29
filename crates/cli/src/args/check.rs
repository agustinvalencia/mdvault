use clap::Args;

#[derive(Debug, Args)]
#[command(after_help = "\
Examples:
  mdv check                             # Run all checks
  mdv check --category broken_references # Run a specific check
  mdv check --json                      # JSON output
  mdv check --quiet                     # Paths only
  mdv check --no-reindex                # Skip index sync check
")]
pub struct CheckArgs {
    /// Run only a specific check category
    #[arg(long, short)]
    pub category: Option<String>,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,

    /// Quiet mode - paths only
    #[arg(long, short)]
    pub quiet: bool,

    /// Skip the index sync check (avoids reindexing)
    #[arg(long)]
    pub no_reindex: bool,
}
