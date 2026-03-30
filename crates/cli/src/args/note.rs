use clap::Args;
use clap_complete::engine::ArgValueCompleter;
use std::path::PathBuf;

use super::{NoteTypeArg, OutputFormat, parse_key_val};

#[derive(Debug, Args)]
#[command(after_help = "\
Examples:
  mdv new task \"My Task\" --var project=myproject
  mdv new --template daily
  mdv new project \"New Project\" --var status=active -o projects/new.md
")]
pub struct NewArgs {
    /// Note type for scaffolding (e.g., "task", "project", "zettel")
    /// Creates a note with frontmatter based on the type's schema
    #[arg(add = ArgValueCompleter::new(crate::completions::complete_types))]
    pub note_type: Option<String>,

    /// Note title (used in frontmatter and as heading)
    pub title: Option<String>,

    /// Use a template file instead of type-based scaffolding
    #[arg(long, add = ArgValueCompleter::new(crate::completions::complete_templates))]
    pub template: Option<String>,

    /// Output file path (auto-generated from type/title if not provided)
    #[arg(long, short)]
    pub output: Option<PathBuf>,

    /// Variables/fields to set (e.g. --var project=myproject)
    #[arg(long = "var", value_parser = parse_key_val)]
    pub vars: Vec<(String, String)>,

    /// Non-interactive mode: fail if variables are missing instead of prompting
    #[arg(long)]
    pub batch: bool,
}

#[derive(Debug, Args)]
#[command(after_help = "\
Examples:
  mdv capture --list
  mdv capture inbox --var text=\"Buy milk\"
  mdv capture todo --var task=\"Review PR\" --var priority=high
")]
pub struct CaptureArgs {
    /// Logical capture name (e.g. "inbox" or "todo")
    #[arg(required_unless_present = "list", add = ArgValueCompleter::new(crate::completions::complete_captures))]
    pub name: Option<String>,

    /// List available captures and their expected variables
    #[arg(long, short)]
    pub list: bool,

    /// Variables to pass to the capture (e.g. --var text="My note")
    #[arg(long = "var", value_parser = parse_key_val)]
    pub vars: Vec<(String, String)>,

    /// Non-interactive mode: fail if variables are missing instead of prompting
    #[arg(long)]
    pub batch: bool,
}

#[derive(Debug, Args)]
#[command(after_help = "\
Examples:
  mdv macro --list
  mdv macro weekly-review
  mdv macro deploy-notes --trust
  mdv macro setup --var project=\"my-app\"
")]
pub struct MacroArgs {
    /// Logical macro name (e.g. "weekly-review" or "deploy")
    #[arg(required_unless_present = "list", add = ArgValueCompleter::new(crate::completions::complete_macros))]
    pub name: Option<String>,

    /// List available macros
    #[arg(long, short)]
    pub list: bool,

    /// Variables to pass to the macro (e.g. --var topic="Planning")
    #[arg(long = "var", value_parser = parse_key_val)]
    pub vars: Vec<(String, String)>,

    /// Non-interactive mode: fail if variables are missing instead of prompting
    #[arg(long)]
    pub batch: bool,

    /// Trust shell commands in the macro
    #[arg(long)]
    pub trust: bool,
}

#[derive(Debug, Args)]
#[command(after_help = "\
Examples:
  mdv list                              # List all notes
  mdv list --type task                  # Filter by type
  mdv list --modified-after 2024-01-01  # Filter by date
  mdv list --modified-after \"today - 7d\" # Notes from last week
  mdv list --json                       # JSON output
  mdv list -q                           # Paths only
")]
pub struct ListArgs {
    /// Filter by note type
    #[arg(long)]
    pub r#type: Option<NoteTypeArg>,

    /// Show only notes modified after this date (YYYY-MM-DD or date expression)
    #[arg(long)]
    pub modified_after: Option<String>,

    /// Show only notes modified before this date (YYYY-MM-DD or date expression)
    #[arg(long)]
    pub modified_before: Option<String>,

    /// Maximum number of notes to return
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
  mdv links note.md                     # Show backlinks and outlinks
  mdv links note.md --backlinks         # Only backlinks
  mdv links note.md --outlinks          # Only outlinks
  mdv links tasks/todo.md --json        # JSON output
")]
pub struct LinksArgs {
    /// Path to the note (relative to vault root)
    #[arg(add = ArgValueCompleter::new(crate::completions::complete_notes))]
    pub note: String,

    /// Show only backlinks (notes linking to this note)
    #[arg(long, short = 'b')]
    pub backlinks: bool,

    /// Show only outgoing links (notes this note links to)
    #[arg(long, short = 'o')]
    pub outlinks: bool,

    /// Output format
    #[arg(long, value_enum, default_value = "table")]
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
  mdv orphans                           # Find orphan notes
  mdv orphans --json                    # JSON output
  mdv orphans -q                        # Paths only
")]
pub struct OrphansArgs {
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
