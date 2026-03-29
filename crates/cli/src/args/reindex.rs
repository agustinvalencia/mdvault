use clap::Args;

#[derive(Debug, Args)]
pub struct ReindexArgs {
    /// Show verbose output (list each file as it's indexed)
    #[arg(long, short)]
    pub verbose: bool,

    /// Force full rebuild of the index (default: incremental)
    #[arg(long)]
    pub force: bool,

    /// Explicitly request incremental update (default behavior)
    #[arg(long, conflicts_with = "force")]
    pub incremental: bool,
}
