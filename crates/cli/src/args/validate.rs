use clap::Args;

use super::OutputFormat;

#[derive(Debug, Args)]
#[command(after_help = "\
Examples:
  mdv validate                          # Validate all notes
  mdv validate path/to/note.md          # Validate specific file
  mdv validate --type task              # Validate only task notes
  mdv validate --fix                    # Auto-fix safe issues
  mdv validate --list-types             # Show available type definitions
  mdv validate --json                   # JSON output
")]
pub struct ValidateArgs {
    /// Specific note path to validate (relative to vault root)
    pub path: Option<String>,

    /// Only validate notes of this type
    #[arg(long)]
    pub r#type: Option<String>,

    /// Maximum number of notes to validate
    #[arg(long, short = 'n')]
    pub limit: Option<u32>,

    /// Auto-fix safe issues (missing defaults, enum case normalization)
    #[arg(long)]
    pub fix: bool,

    /// List available type definitions
    #[arg(long)]
    pub list_types: bool,

    /// Output format
    #[arg(long, short, value_enum, default_value = "table")]
    pub output: OutputFormat,

    /// Output as JSON (shorthand for --output json)
    #[arg(long)]
    pub json: bool,

    /// Quiet mode - output paths only (shorthand for --output quiet)
    #[arg(long, short)]
    pub quiet: bool,

    /// Check link integrity (report broken links as warnings)
    #[arg(long)]
    pub check_links: bool,
}
