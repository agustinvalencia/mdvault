mod cmd;

use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "markadd", version, about = "Terminal-first Markdown automation")]
struct Cli {
    #[arg(long, global = true)]
    config: Option<PathBuf>,

    #[arg(long, global = true)]
    profile: Option<String>,

    #[command(subcommand)]
    command: Commands,
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
}

#[derive(Debug, Args)]
pub struct NewArgs {
    /// Logical template name (e.g. "daily" or "blog/post")
    #[arg(long)]
    pub template: String,

    /// Output file path to create
    #[arg(long)]
    pub output: PathBuf,
}

#[derive(Debug, Args)]
#[command(after_help = "\
Examples:
  markadd capture --list
  markadd capture inbox --var text=\"Buy milk\"
  markadd capture todo --var task=\"Review PR\" --var priority=high
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
}

fn parse_key_val(s: &str) -> Result<(String, String), String> {
    let pos =
        s.find('=').ok_or_else(|| format!("invalid KEY=value: no `=` found in `{s}`"))?;
    Ok((s[..pos].to_string(), s[pos + 1..].to_string()))
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Doctor => {
            cmd::doctor::run(cli.config.as_deref(), cli.profile.as_deref())
        }
        Commands::ListTemplates => {
            cmd::list_templates::run(cli.config.as_deref(), cli.profile.as_deref())
        }
        Commands::New(args) => {
            cmd::new::run(
                cli.config.as_deref(),
                cli.profile.as_deref(),
                &args.template,
                &args.output,
            );
        }
        Commands::Capture(args) => {
            if args.list {
                cmd::capture::run_list(cli.config.as_deref(), cli.profile.as_deref());
            } else {
                cmd::capture::run(
                    cli.config.as_deref(),
                    cli.profile.as_deref(),
                    args.name.as_ref().unwrap(),
                    &args.vars,
                );
            }
        }
    }
}
