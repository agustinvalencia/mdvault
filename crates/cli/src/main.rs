mod cmd;

use clap::{Parser, Subcommand};
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
    Doctor,
    ListTemplates,
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
    }
}
