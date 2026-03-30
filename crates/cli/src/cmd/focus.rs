//! Focus command: manage active project context.
//!
//! The focus command sets, shows, or clears the active project context.
//! This context is used by other commands to provide smart defaults.

use color_eyre::eyre::{Result, WrapErr};
use mdvault_core::activity::ActivityLogService;
use mdvault_core::context::ContextManager;

use super::common::load_config;
use crate::FocusArgs;

/// Run the focus command.
pub fn run(
    config_path: Option<&std::path::Path>,
    profile: Option<&str>,
    args: FocusArgs,
) -> Result<()> {
    let cfg = load_config(config_path, profile)?;

    let mut manager =
        ContextManager::load(&cfg.vault_root).wrap_err("Failed to load context state")?;

    // Handle --clear flag
    if args.clear {
        // Get current project for logging before clearing
        let prev_project = manager.active_project().map(|s| s.to_string());

        manager.clear_focus().wrap_err("Failed to clear focus")?;

        // Log to activity log
        if let Some(activity) = ActivityLogService::try_from_config(&cfg) {
            if let Some(ref project) = prev_project {
                let _ = activity.log_focus(project, None, "clear");
            }
        }

        println!("Focus cleared.");
        return Ok(());
    }

    // Handle setting focus
    if let Some(project) = &args.project {
        let result = if let Some(note) = &args.note {
            manager.set_focus_with_note(project, note)
        } else {
            manager.set_focus(project)
        };

        result.wrap_err("Failed to set focus")?;

        // Log to activity log
        if let Some(activity) = ActivityLogService::try_from_config(&cfg) {
            let _ = activity.log_focus(project, args.note.as_deref(), "set");
        }

        println!("Focus set to: {}", project);
        if let Some(note) = &args.note {
            println!("Note: {}", note);
        }
        return Ok(());
    }

    // No arguments: show current focus
    if args.json {
        let state = manager.state();
        let json =
            serde_json::to_string_pretty(state).wrap_err("Failed to serialize state")?;
        println!("{}", json);
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

    Ok(())
}
