pub mod area;
pub mod check;
pub mod completions_args;
pub mod context;
pub mod dashboard;
pub mod focus;
pub mod note;
pub mod project;
pub mod reindex;
pub mod rename;
pub mod report;
pub mod search;
pub mod task;
pub mod validate;

use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

pub use self::area::*;
pub use self::check::*;
pub use self::completions_args::*;
pub use self::context::*;
pub use self::dashboard::*;
pub use self::focus::*;
pub use self::note::*;
pub use self::project::*;
pub use self::reindex::*;
pub use self::rename::*;
pub use self::report::*;
pub use self::search::*;
pub use self::task::*;
pub use self::validate::*;

/// Output format for query commands.
#[derive(Debug, Clone, Copy, Default, ValueEnum)]
pub enum OutputFormat {
    /// Human-readable table format
    #[default]
    Table,
    /// JSON output
    Json,
    /// Quiet mode - paths only
    Quiet,
}

/// Task/project status filter.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum StatusFilter {
    /// Not started
    Todo,
    /// Currently being worked on
    #[value(alias = "doing")]
    InProgress,
    /// Waiting on something
    #[value(alias = "waiting")]
    Blocked,
    /// Completed
    #[value(alias = "completed")]
    Done,
    /// No longer needed
    #[value(alias = "canceled")]
    Cancelled,
}

impl StatusFilter {
    /// Return all normalised status strings that match this filter variant.
    pub fn matches(&self, status: &str) -> bool {
        match self {
            Self::Todo => matches!(status, "todo" | "open"),
            Self::InProgress => matches!(status, "in-progress" | "in_progress" | "doing"),
            Self::Blocked => matches!(status, "blocked" | "waiting"),
            Self::Done => matches!(status, "done" | "completed"),
            Self::Cancelled => matches!(status, "cancelled" | "canceled"),
        }
    }
}

/// Project kind filter.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum KindFilter {
    /// A project with a defined end
    Project,
    /// An ongoing area of responsibility
    Area,
}

impl KindFilter {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Area => "area",
        }
    }
}

/// Note type filter for list command.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum NoteTypeArg {
    /// Daily journal notes
    Daily,
    /// Weekly overview notes
    Weekly,
    /// Individual actionable tasks
    Task,
    /// Collections of related tasks
    Project,
    /// Knowledge notes (Zettelkasten-style)
    Zettel,
}

impl From<NoteTypeArg> for mdvault_core::index::NoteType {
    fn from(arg: NoteTypeArg) -> Self {
        match arg {
            NoteTypeArg::Daily => mdvault_core::index::NoteType::Daily,
            NoteTypeArg::Weekly => mdvault_core::index::NoteType::Weekly,
            NoteTypeArg::Task => mdvault_core::index::NoteType::Task,
            NoteTypeArg::Project => mdvault_core::index::NoteType::Project,
            NoteTypeArg::Zettel => mdvault_core::index::NoteType::Zettel,
        }
    }
}

#[derive(Debug, Parser)]
#[command(name = "mdv", version, about = "Your markdown vault on the command line")]
pub struct Cli {
    #[arg(long, global = true)]
    pub config: Option<PathBuf>,

    #[arg(long, global = true)]
    pub profile: Option<String>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Validate configuration and print resolved paths
    Doctor,

    /// List logical template names discovered under templates_dir
    ListTemplates,

    /// Render a template into a new file
    New(NewArgs),

    /// Capture content into an existing file's section
    Capture(CaptureArgs),

    /// Execute a multi-step macro workflow
    Macro(MacroArgs),

    /// Build or rebuild the vault index
    Reindex(ReindexArgs),

    /// List notes in the vault with optional filters
    List(ListArgs),

    /// Show links for a note (backlinks and/or outgoing)
    Links(LinksArgs),

    /// Find orphan notes (alias for stale --orphans)
    #[command(hide = true)]
    Orphans(OrphansArgs),

    /// Validate notes against type definitions
    Validate(ValidateArgs),

    /// Search notes with contextual expansion
    Search(SearchArgs),

    /// Find unused notes (stale or orphaned)
    Stale(StaleArgs),

    /// Rename a note and update all references to it
    Rename(RenameArgs),

    /// Generate shell completion scripts
    Completions(CompletionsArgs),

    /// Task management commands
    #[command(subcommand)]
    Task(TaskCommands),

    /// Project management commands
    #[command(subcommand)]
    Project(ProjectCommands),

    /// Area management commands
    #[command(subcommand)]
    Area(AreaCommands),

    /// Generate activity reports for a time period
    Report(ReportArgs),

    /// Daily planning and review dashboard
    Today(TodayArgs),

    /// Set or show active focus context
    Focus(FocusArgs),

    /// Query context for a day or week
    #[command(subcommand)]
    Context(ContextCommands),

    /// Interactive dashboard TUI
    Dashboard(DashboardArgs),

    /// Check vault structural correctness (lint)
    Check(CheckArgs),
}

pub(crate) fn parse_key_val(s: &str) -> Result<(String, String), String> {
    let pos =
        s.find('=').ok_or_else(|| format!("invalid KEY=value: no `=` found in `{s}`"))?;
    Ok((s[..pos].to_string(), s[pos + 1..].to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_filter_matches_canonical() {
        assert!(StatusFilter::Todo.matches("todo"));
        assert!(StatusFilter::InProgress.matches("in-progress"));
        assert!(StatusFilter::InProgress.matches("in_progress"));
        assert!(StatusFilter::Blocked.matches("blocked"));
        assert!(StatusFilter::Done.matches("done"));
        assert!(StatusFilter::Cancelled.matches("cancelled"));
    }

    #[test]
    fn status_filter_matches_aliases() {
        assert!(StatusFilter::Todo.matches("open"));
        assert!(StatusFilter::InProgress.matches("doing"));
        assert!(StatusFilter::Blocked.matches("waiting"));
        assert!(StatusFilter::Done.matches("completed"));
        assert!(StatusFilter::Cancelled.matches("canceled"));
    }

    #[test]
    fn status_filter_rejects_wrong_status() {
        assert!(!StatusFilter::Todo.matches("done"));
        assert!(!StatusFilter::Done.matches("todo"));
        assert!(!StatusFilter::InProgress.matches("blocked"));
    }

    #[test]
    fn kind_filter_as_str() {
        assert_eq!(KindFilter::Project.as_str(), "project");
        assert_eq!(KindFilter::Area.as_str(), "area");
    }
}
