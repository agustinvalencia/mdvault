use clap::{Args, ValueEnum};

use super::{NoteTypeArg, OutputFormat};

/// Search mode for result expansion.
#[derive(Debug, Clone, Copy, Default, ValueEnum)]
pub enum SearchModeArg {
    /// Only return notes directly matching the query
    #[default]
    Direct,
    /// Include linked notes within 2 hops
    Neighbourhood,
    /// Include recent dailies referencing matches
    Temporal,
    /// Include notes that cooccur with matches
    Cooccurrence,
    /// Full contextual search (all modes)
    Full,
}

#[derive(Debug, Args)]
#[command(after_help = "\
Examples:
  mdv search \"parser\"                     # Direct search for 'parser'
  mdv search \"parser\" --mode full         # Search with full context
  mdv search \"fix bug\" --type task        # Search only task notes
  mdv search --type task --mode full       # All tasks with context
  mdv search \"ML\" --boost                 # Boost recently active notes
")]
pub struct SearchArgs {
    /// Search query (matches title and path)
    pub query: Option<String>,

    /// Filter by note type
    #[arg(long)]
    pub r#type: Option<NoteTypeArg>,

    /// Search mode for context expansion
    #[arg(long, value_enum, default_value = "direct")]
    pub mode: SearchModeArg,

    /// Boost recently active notes
    #[arg(long)]
    pub boost: bool,

    /// Maximum number of results
    #[arg(long, short = 'n')]
    pub limit: Option<u32>,

    /// Output format
    #[arg(long, short, value_enum, default_value = "table")]
    pub output: OutputFormat,

    /// Output as JSON (shorthand for --output json)
    #[arg(long)]
    pub json: bool,

    /// Quiet mode - output paths only (shorthand for --output quiet)
    #[arg(long, short)]
    pub quiet: bool,
}

#[derive(Debug, Args)]
#[command(after_help = "\
Examples:
  mdv stale                              # List all stale notes
  mdv stale --type task                  # Only stale tasks
  mdv stale --threshold 0.7              # Higher staleness threshold
  mdv stale --days 90                    # Notes not seen in 90 days
  mdv stale --orphans                    # Find notes with no incoming links
")]
pub struct StaleArgs {
    /// Find orphan notes (no incoming links) instead of stale notes
    #[arg(long)]
    pub orphans: bool,

    /// Filter by note type
    #[arg(long)]
    pub r#type: Option<NoteTypeArg>,

    /// Minimum staleness score (0.0-1.0, default 0.5)
    #[arg(long, default_value = "0.5")]
    pub threshold: f64,

    /// Show notes not seen for this many days (alternative to threshold)
    #[arg(long)]
    pub days: Option<u32>,

    /// Maximum number of results
    #[arg(long, short = 'n')]
    pub limit: Option<u32>,

    /// Output format
    #[arg(long, short, value_enum, default_value = "table")]
    pub output: OutputFormat,

    /// Output as JSON (shorthand for --output json)
    #[arg(long)]
    pub json: bool,

    /// Quiet mode - output paths only (shorthand for --output quiet)
    #[arg(long, short)]
    pub quiet: bool,
}
