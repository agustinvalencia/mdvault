mod cmd;
mod prompt;
mod tui;

use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

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
struct Cli {
    #[arg(long, global = true)]
    config: Option<PathBuf>,

    #[arg(long, global = true)]
    profile: Option<String>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
enum Commands {
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

    /// Find orphan notes (no incoming links)
    Orphans(OrphansArgs),
}

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

#[derive(Debug, Args)]
#[command(after_help = "\
Examples:
  mdv macro --list
  mdv macro weekly-review
  mdv macro deploy-notes --trust
  mdv macro setup --var project=\"my-app\"
")]
pub struct MacroArgs {
    /// Logical macro name (e.g. \"weekly-review\" or \"deploy\")
    #[arg(required_unless_present = "list")]
    pub name: Option<String>,

    /// List available macros
    #[arg(long, short)]
    pub list: bool,

    /// Variables to pass to the macro (e.g. --var topic=\"Planning\")
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
pub struct NewArgs {
    /// Logical template name (e.g. "daily" or "blog/post")
    #[arg(long)]
    pub template: String,

    /// Output file path to create (optional if template defines output in frontmatter)
    #[arg(long)]
    pub output: Option<PathBuf>,

    /// Variables to pass to the template (e.g. --var title="My Note")
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
    #[arg(required_unless_present = "list")]
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

fn parse_key_val(s: &str) -> Result<(String, String), String> {
    let pos =
        s.find('=').ok_or_else(|| format!("invalid KEY=value: no `=` found in `{s}`"))?;
    Ok((s[..pos].to_string(), s[pos + 1..].to_string()))
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        // No command provided - launch TUI
        None => {
            if let Err(e) = tui::run(cli.config.as_deref(), cli.profile.as_deref()) {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
        Some(Commands::Doctor) => {
            cmd::doctor::run(cli.config.as_deref(), cli.profile.as_deref())
        }
        Some(Commands::ListTemplates) => {
            cmd::list_templates::run(cli.config.as_deref(), cli.profile.as_deref())
        }
        Some(Commands::New(args)) => {
            cmd::new::run(
                cli.config.as_deref(),
                cli.profile.as_deref(),
                &args.template,
                args.output.as_deref(),
                &args.vars,
                args.batch,
            );
        }
        Some(Commands::Capture(args)) => {
            if args.list {
                cmd::capture::run_list(cli.config.as_deref(), cli.profile.as_deref());
            } else {
                cmd::capture::run(
                    cli.config.as_deref(),
                    cli.profile.as_deref(),
                    args.name.as_ref().unwrap(),
                    &args.vars,
                    args.batch,
                );
            }
        }
        Some(Commands::Macro(args)) => {
            if args.list {
                cmd::macro_cmd::run_list(cli.config.as_deref(), cli.profile.as_deref());
            } else {
                cmd::macro_cmd::run(
                    cli.config.as_deref(),
                    cli.profile.as_deref(),
                    args.name.as_ref().unwrap(),
                    &args.vars,
                    args.batch,
                    args.trust,
                );
            }
        }
        Some(Commands::Reindex(args)) => {
            cmd::reindex::run(
                cli.config.as_deref(),
                cli.profile.as_deref(),
                args.verbose,
                args.force,
            );
        }
        Some(Commands::List(args)) => {
            cmd::list::run(cli.config.as_deref(), cli.profile.as_deref(), args);
        }
        Some(Commands::Links(args)) => {
            cmd::links::run(cli.config.as_deref(), cli.profile.as_deref(), args);
        }
        Some(Commands::Orphans(args)) => {
            cmd::orphans::run(cli.config.as_deref(), cli.profile.as_deref(), args);
        }
    }
}
