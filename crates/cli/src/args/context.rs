use clap::{Args, Subcommand};

/// Context query subcommands.
#[derive(Debug, Subcommand)]
pub enum ContextCommands {
    /// Get context for a specific day
    Day(ContextDayArgs),
    /// Get context for a specific week
    Week(ContextWeekArgs),
    /// Get context for a specific note
    Note(ContextNoteArgs),
    /// Get context for the focused project
    Focus(ContextFocusArgs),
}

#[derive(Debug, Args)]
#[command(after_help = "\
Examples:
  mdv context day                     # Today's context
  mdv context day yesterday           # Yesterday's context
  mdv context day 2026-01-20          # Specific date
  mdv context day \"today - 3d\"        # Date expression
  mdv context day --format json       # JSON output
")]
pub struct ContextDayArgs {
    /// Date (YYYY-MM-DD, "today", "yesterday", or date expression)
    pub date: Option<String>,

    /// Output format (md, json, summary)
    #[arg(long, default_value = "md")]
    pub format: String,

    /// Find last day with activity if specified date has none
    #[arg(long)]
    pub lookback: bool,
}

#[derive(Debug, Args)]
#[command(after_help = "\
Examples:
  mdv context week                    # Current week's context
  mdv context week last               # Last week
  mdv context week 2026-W04           # Specific ISO week
  mdv context week --format json      # JSON output
")]
pub struct ContextWeekArgs {
    /// Week ("current", "last", YYYY-Wxx, or date expression)
    pub week: Option<String>,

    /// Output format (md, json, summary)
    #[arg(long, default_value = "md")]
    pub format: String,
}

#[derive(Debug, Args)]
#[command(after_help = "\
Examples:
  mdv context note Projects/foo/foo.md    # Context for a project
  mdv context note tasks/TST-001.md       # Context for a task
  mdv context note --format json path.md  # JSON output
  mdv context note --activity-days 14     # Include 14 days of activity
")]
pub struct ContextNoteArgs {
    /// Path to the note (relative to vault root)
    pub path: String,

    /// Output format (md, json, summary)
    #[arg(long, default_value = "md")]
    pub format: String,

    /// Days of activity history to include
    #[arg(long, default_value = "7")]
    pub activity_days: u32,
}

#[derive(Debug, Args)]
#[command(after_help = "\
Examples:
  mdv context focus                       # Show focused project context
  mdv context focus --format json         # JSON output
  mdv context focus --with-tasks          # Include full task list
")]
pub struct ContextFocusArgs {
    /// Output format (md, json, summary)
    #[arg(long, default_value = "md")]
    pub format: String,

    /// Include full task list
    #[arg(long)]
    pub with_tasks: bool,
}
