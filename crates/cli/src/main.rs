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
    }
}
