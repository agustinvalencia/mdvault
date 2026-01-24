mod cmd;
mod completions;
mod logging;
mod prompt;
mod tui;

use clap::{Args, CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::engine::ArgValueCompleter;
use clap_complete::env::CompleteEnv;
use clap_complete::Shell;
use mdvault_core::config::loader::ConfigLoader;
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

    /// Validate notes against type definitions
    Validate(ValidateArgs),

    /// Lint notes (alias for validate)
    #[command(hide = true)]
    Lint(ValidateArgs),

    /// Search notes with contextual expansion
    Search(SearchArgs),

    /// Find stale notes (not referenced in recent dailies)
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

    /// Generate activity reports for a time period
    Report(ReportArgs),

    /// Daily planning and review dashboard
    Today(TodayArgs),

    /// Set or show active focus context
    Focus(FocusArgs),

    /// Query context for a day or week
    #[command(subcommand)]
    Context(ContextCommands),
}

/// Today command subcommands.
#[derive(Debug, Subcommand)]
pub enum TodayCommands {
    /// Open today's daily note in default editor
    Open,
}

#[derive(Debug, Args)]
#[command(after_help = "\
Examples:
  mdv today                         # Smart dashboard (auto-selects plan/review based on time)
  mdv today --plan                  # Force morning planning mode
  mdv today --review                # Force evening review mode
  mdv today open                    # Open today's daily note in $EDITOR
")]
pub struct TodayArgs {
    /// Force morning planning mode
    #[arg(long, conflicts_with = "review")]
    pub plan: bool,

    /// Force evening review mode
    #[arg(long, conflicts_with = "plan")]
    pub review: bool,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,

    #[command(subcommand)]
    pub command: Option<TodayCommands>,
}

/// Task management subcommands.
#[derive(Debug, Subcommand)]
enum TaskCommands {
    /// List tasks with optional filters
    List(TaskListArgs),

    /// Mark a task as done
    Done(TaskDoneArgs),

    /// Show detailed status for a task
    Status(TaskStatusArgs),
}

/// Project management subcommands.
#[derive(Debug, Subcommand)]
enum ProjectCommands {
    /// List all projects with task counts
    List(ProjectListArgs),

    /// Show project status with tasks in kanban-style view
    Status(ProjectStatusArgs),

    /// Show project progress with completion metrics and velocity
    Progress(ProjectProgressArgs),
}

#[derive(Debug, Args)]
pub struct TaskListArgs {
    /// Filter by project name
    #[arg(long, short)]
    pub project: Option<String>,

    /// Filter by status (todo, in-progress, done, blocked)
    #[arg(long, short)]
    pub status: Option<String>,
}

#[derive(Debug, Args)]
pub struct TaskDoneArgs {
    /// Path to the task note (relative to vault root)
    #[arg(add = ArgValueCompleter::new(completions::complete_notes))]
    pub task: PathBuf,

    /// Summary of what was done (logged to task)
    #[arg(long, short)]
    pub summary: Option<String>,
}

#[derive(Debug, Args)]
pub struct ProjectListArgs {
    /// Filter by status (active, completed, on-hold, archived)
    #[arg(long, short)]
    pub status: Option<String>,
}

#[derive(Debug, Args)]
pub struct ProjectStatusArgs {
    /// Project ID or folder name (e.g., "MCP" or "my-cool-project")
    #[arg(add = ArgValueCompleter::new(completions::complete_projects))]
    pub project: String,
}

#[derive(Debug, Args)]
pub struct ProjectProgressArgs {
    /// Project ID or folder name (optional - shows all projects if omitted)
    #[arg(add = ArgValueCompleter::new(completions::complete_projects))]
    pub project: Option<String>,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,

    /// Include archived projects
    #[arg(long)]
    pub include_archived: bool,
}

#[derive(Debug, Args)]
pub struct TaskStatusArgs {
    /// Task ID (e.g., "MCP-001")
    #[arg(add = ArgValueCompleter::new(completions::complete_notes))]
    pub task_id: String,
}

#[derive(Debug, Args)]
pub struct ReportArgs {
    /// Generate report for a specific month (YYYY-MM format)
    #[arg(long, conflicts_with = "week")]
    pub month: Option<String>,

    /// Generate report for a specific week (YYYY-WXX format)
    #[arg(long, conflicts_with = "month")]
    pub week: Option<String>,

    /// Output report to a markdown file instead of terminal
    #[arg(long, short)]
    pub output: Option<std::path::PathBuf>,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
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
    #[arg(required_unless_present = "list", add = ArgValueCompleter::new(completions::complete_macros))]
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
#[command(after_help = "\
Examples:
  mdv new task \"My Task\" --var project=myproject
  mdv new --template daily
  mdv new project \"New Project\" --var status=active -o projects/new.md
")]
pub struct NewArgs {
    /// Note type for scaffolding (e.g., \"task\", \"project\", \"zettel\")
    /// Creates a note with frontmatter based on the type's schema
    #[arg(add = ArgValueCompleter::new(completions::complete_types))]
    pub note_type: Option<String>,

    /// Note title (used in frontmatter and as heading)
    pub title: Option<String>,

    /// Use a template file instead of type-based scaffolding
    #[arg(long, add = ArgValueCompleter::new(completions::complete_templates))]
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
    #[arg(required_unless_present = "list", add = ArgValueCompleter::new(completions::complete_captures))]
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
    #[arg(add = ArgValueCompleter::new(completions::complete_notes))]
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
")]
pub struct StaleArgs {
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

#[derive(Debug, Args)]
#[command(after_help = "\
Examples:
  mdv rename old.md new.md              # Rename note and update references
  mdv rename old.md new.md --dry-run    # Preview changes without modifying files
  mdv rename old.md new.md --yes        # Skip confirmation prompt
")]
pub struct RenameArgs {
    /// Source file path (relative to vault root)
    #[arg(add = ArgValueCompleter::new(completions::complete_notes))]
    pub source: std::path::PathBuf,

    /// Destination file path (relative to vault root)
    pub dest: std::path::PathBuf,

    /// Preview changes without modifying files
    #[arg(long)]
    pub dry_run: bool,

    /// Skip confirmation prompt
    #[arg(long, short)]
    pub yes: bool,
}

#[derive(Debug, Args)]
#[command(after_help = "\
Examples:
  mdv completions bash > ~/.local/share/bash-completion/completions/mdv
  mdv completions zsh > ~/.zfunc/_mdv
  mdv completions fish > ~/.config/fish/completions/mdv.fish
")]
pub struct CompletionsArgs {
    /// Shell to generate completions for
    #[arg(value_enum)]
    pub shell: Shell,
}

#[derive(Debug, Args)]
#[command(after_help = "\
Examples:
  mdv focus                           # Show current focus
  mdv focus MCP                       # Set focus to project MCP
  mdv focus MCP --note \"OAuth work\"   # Set focus with note
  mdv focus --clear                   # Clear focus
")]
pub struct FocusArgs {
    /// Project ID to focus on (e.g., \"MCP\", \"VAULT\")
    #[arg(add = ArgValueCompleter::new(completions::complete_projects))]
    pub project: Option<String>,

    /// Note describing current work
    #[arg(long, short)]
    pub note: Option<String>,

    /// Clear the current focus
    #[arg(long, short)]
    pub clear: bool,

    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

/// Context query subcommands.
#[derive(Debug, Subcommand)]
pub enum ContextCommands {
    /// Get context for a specific day
    Day(ContextDayArgs),
    /// Get context for a specific week
    Week(ContextWeekArgs),
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
    /// Date (YYYY-MM-DD, \"today\", \"yesterday\", or date expression)
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
    /// Week (\"current\", \"last\", YYYY-Wxx, or date expression)
    pub week: Option<String>,

    /// Output format (md, json, summary)
    #[arg(long, default_value = "md")]
    pub format: String,
}

fn parse_key_val(s: &str) -> Result<(String, String), String> {
    let pos =
        s.find('=').ok_or_else(|| format!("invalid KEY=value: no `=` found in `{s}`"))?;
    Ok((s[..pos].to_string(), s[pos + 1..].to_string()))
}

fn main() {
    // Enable dynamic shell completions
    // This intercepts completion requests before normal CLI parsing
    CompleteEnv::with_factory(Cli::command).complete();

    let cli = Cli::parse();

    // Initialize logging if config is valid
    // We ignore errors here because individual commands will report them properly
    if let Ok(cfg) = ConfigLoader::load(cli.config.as_deref(), cli.profile.as_deref()) {
        logging::init(&cfg);
    }

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
            cmd::new::run(cli.config.as_deref(), cli.profile.as_deref(), args);
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
        Some(Commands::Validate(args)) | Some(Commands::Lint(args)) => {
            cmd::validate::run(cli.config.as_deref(), cli.profile.as_deref(), args);
        }
        Some(Commands::Search(args)) => {
            cmd::search::run(cli.config.as_deref(), cli.profile.as_deref(), args);
        }
        Some(Commands::Stale(args)) => {
            cmd::stale::run(cli.config.as_deref(), cli.profile.as_deref(), args);
        }
        Some(Commands::Rename(args)) => {
            cmd::rename::run(cli.config.as_deref(), cli.profile.as_deref(), args);
        }
        Some(Commands::Completions(args)) => {
            clap_complete::generate(
                args.shell,
                &mut Cli::command(),
                "mdv",
                &mut std::io::stdout(),
            );
        }
        Some(Commands::Task(subcmd)) => match subcmd {
            TaskCommands::List(args) => {
                cmd::task::list(
                    cli.config.as_deref(),
                    cli.profile.as_deref(),
                    args.project.as_deref(),
                    args.status.as_deref(),
                );
            }
            TaskCommands::Done(args) => {
                cmd::task::done(
                    cli.config.as_deref(),
                    cli.profile.as_deref(),
                    &args.task,
                    args.summary.as_deref(),
                );
            }
            TaskCommands::Status(args) => {
                cmd::task::status(
                    cli.config.as_deref(),
                    cli.profile.as_deref(),
                    &args.task_id,
                );
            }
        },
        Some(Commands::Project(subcmd)) => match subcmd {
            ProjectCommands::List(args) => {
                cmd::project::list(
                    cli.config.as_deref(),
                    cli.profile.as_deref(),
                    args.status.as_deref(),
                );
            }
            ProjectCommands::Status(args) => {
                cmd::project::status(
                    cli.config.as_deref(),
                    cli.profile.as_deref(),
                    &args.project,
                );
            }
            ProjectCommands::Progress(args) => {
                cmd::project::progress(
                    cli.config.as_deref(),
                    cli.profile.as_deref(),
                    args.project.as_deref(),
                    args.json,
                    args.include_archived,
                );
            }
        },
        Some(Commands::Report(args)) => {
            cmd::report::run(
                cli.config.as_deref(),
                cli.profile.as_deref(),
                args.month.as_deref(),
                args.week.as_deref(),
                args.output.as_deref(),
                args.json,
            );
        }
        Some(Commands::Today(args)) => {
            cmd::today::run(cli.config.as_deref(), cli.profile.as_deref(), args);
        }
        Some(Commands::Focus(args)) => {
            cmd::focus::run(cli.config.as_deref(), cli.profile.as_deref(), args);
        }
        Some(Commands::Context(subcmd)) => match subcmd {
            ContextCommands::Day(args) => {
                cmd::context::day(
                    cli.config.as_deref(),
                    cli.profile.as_deref(),
                    args.date.as_deref(),
                    &args.format,
                    args.lookback,
                );
            }
            ContextCommands::Week(args) => {
                cmd::context::week(
                    cli.config.as_deref(),
                    cli.profile.as_deref(),
                    args.week.as_deref(),
                    &args.format,
                );
            }
        },
    }
}
