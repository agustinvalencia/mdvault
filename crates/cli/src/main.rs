mod args;
mod cmd;
mod completions;
mod logging;
mod prompt;
mod tui;

pub use args::*;

use clap::{CommandFactory, Parser};
use clap_complete::env::CompleteEnv;
use mdvault_core::config::loader::ConfigLoader;

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
            // Hidden alias: convert to stale --orphans
            let stale_args = StaleArgs {
                orphans: true,
                r#type: None,
                threshold: 0.5,
                days: None,
                limit: None,
                output: args.output,
                json: args.json,
                quiet: args.quiet,
            };
            cmd::stale::run(cli.config.as_deref(), cli.profile.as_deref(), stale_args);
        }
        Some(Commands::Validate(args)) => {
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
                    args.status,
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
            TaskCommands::Cancel(args) => {
                cmd::task::cancel(
                    cli.config.as_deref(),
                    cli.profile.as_deref(),
                    &args.task,
                    args.reason.as_deref(),
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
                    args.status,
                    args.kind,
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
            ProjectCommands::Archive(args) => {
                cmd::project::archive(
                    cli.config.as_deref(),
                    cli.profile.as_deref(),
                    &args.project,
                    args.yes,
                );
            }
        },
        Some(Commands::Area(subcmd)) => match subcmd {
            AreaCommands::Report(args) => {
                cmd::area::report(
                    cli.config.as_deref(),
                    cli.profile.as_deref(),
                    &args.area,
                    &args.period,
                    args.json,
                );
            }
            AreaCommands::Export(args) => {
                cmd::area::export(
                    cli.config.as_deref(),
                    cli.profile.as_deref(),
                    &args.area,
                    args.from.as_deref(),
                    args.to.as_deref(),
                    &args.format,
                );
            }
        },
        Some(Commands::Report(args)) => {
            if args.visual || args.dashboard {
                cmd::report::run_dashboard(
                    cli.config.as_deref(),
                    cli.profile.as_deref(),
                    args.project.as_deref(),
                    args.activity_days,
                    args.json,
                    args.output.as_deref(),
                    args.visual,
                );
            } else {
                cmd::report::run(
                    cli.config.as_deref(),
                    cli.profile.as_deref(),
                    args.month.as_deref(),
                    args.week.as_deref(),
                    args.output.as_deref(),
                    args.json,
                );
            }
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
            ContextCommands::Note(args) => {
                cmd::context::note(
                    cli.config.as_deref(),
                    cli.profile.as_deref(),
                    &args.path,
                    &args.format,
                    args.activity_days,
                );
            }
            ContextCommands::Focus(args) => {
                cmd::context::focus(
                    cli.config.as_deref(),
                    cli.profile.as_deref(),
                    &args.format,
                    args.with_tasks,
                );
            }
        },
        Some(Commands::Check(args)) => {
            cmd::check::run(cli.config.as_deref(), cli.profile.as_deref(), args);
        }
        Some(Commands::Dashboard(args)) => {
            if let Err(e) = tui::dashboard::run(
                cli.config.as_deref(),
                cli.profile.as_deref(),
                args.project.as_deref(),
                args.activity_days,
            ) {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
        }
    }
}
