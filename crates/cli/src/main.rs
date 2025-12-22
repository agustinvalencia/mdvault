mod cmd;
mod prompt;
mod tui;

use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

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
    }
}
