//! Focus command: manage active project context.
//!
//! The focus command sets, shows, or clears the active project context.
//! This context is used by other commands to provide smart defaults.

use mdvault_core::config::loader::ConfigLoader;
use mdvault_core::context::ContextManager;

use crate::FocusArgs;

/// Run the focus command.
pub fn run(
    config_path: Option<&std::path::Path>,
    profile: Option<&str>,
    args: FocusArgs,
) {
    let cfg = match ConfigLoader::load(config_path, profile) {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("Configuration error: {e}");
            std::process::exit(1);
        }
    };

    let mut manager = match ContextManager::load(&cfg.vault_root) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Failed to load context state: {e}");
            std::process::exit(1);
        }
    };

    // Handle --clear flag
    if args.clear {
        if let Err(e) = manager.clear_focus() {
            eprintln!("Failed to clear focus: {e}");
            std::process::exit(1);
        }
        println!("Focus cleared.");
        return;
    }

    // Handle setting focus
    if let Some(project) = &args.project {
        let result = if let Some(note) = &args.note {
            manager.set_focus_with_note(project, note)
        } else {
            manager.set_focus(project)
        };

        if let Err(e) = result {
            eprintln!("Failed to set focus: {e}");
            std::process::exit(1);
        }

        println!("Focus set to: {}", project);
        if let Some(note) = &args.note {
            println!("Note: {}", note);
        }
        return;
    }

    // No arguments: show current focus
    if args.json {
        let state = manager.state();
        match serde_json::to_string_pretty(state) {
            Ok(json) => println!("{}", json),
            Err(e) => {
                eprintln!("Failed to serialize state: {e}");
                std::process::exit(1);
            }
        }
    } else {
        match manager.focus() {
            Some(focus) => {
                println!("Active focus: {}", focus.project);
                if let Some(note) = &focus.note {
                    println!("Note: {}", note);
                }
                if let Some(started) = &focus.started_at {
                    println!("Since: {}", started.format("%Y-%m-%d %H:%M"));
                }
            }
            None => {
                println!("No active focus.");
                println!("Use 'mdv focus <PROJECT>' to set focus.");
            }
        }
    }
}
