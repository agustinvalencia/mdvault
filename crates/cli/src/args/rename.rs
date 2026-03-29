use clap::Args;
use clap_complete::engine::ArgValueCompleter;
use std::path::PathBuf;

#[derive(Debug, Args)]
#[command(after_help = "\
Examples:
  mdv rename old.md new.md              # Rename note and update references
  mdv rename old.md new.md --dry-run    # Preview changes without modifying files
  mdv rename old.md new.md --yes        # Skip confirmation prompt
")]
pub struct RenameArgs {
    /// Source file path (relative to vault root)
    #[arg(add = ArgValueCompleter::new(crate::completions::complete_notes))]
    pub source: PathBuf,

    /// Destination file path (relative to vault root)
    pub dest: PathBuf,

    /// Preview changes without modifying files
    #[arg(long)]
    pub dry_run: bool,

    /// Skip confirmation prompt
    #[arg(long, short)]
    pub yes: bool,
}
