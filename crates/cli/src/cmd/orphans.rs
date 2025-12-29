//! Orphans command implementation.

use std::path::Path;

use mdvault_core::config::loader::ConfigLoader;
use mdvault_core::index::IndexDb;

use super::output::{print_notes_json, print_notes_quiet, print_notes_table};
use crate::{OrphansArgs, OutputFormat};

pub fn run(config: Option<&Path>, profile: Option<&str>, args: OrphansArgs) {
    // Load configuration
    let rc = match ConfigLoader::load(config, profile) {
        Ok(rc) => rc,
        Err(e) => {
            eprintln!("Error loading config: {}", e);
            std::process::exit(1);
        }
    };

    // Open database
    let index_path = rc.vault_root.join(".mdvault/index.db");
    let db = match IndexDb::open(&index_path) {
        Ok(db) => db,
        Err(e) => {
            eprintln!("Error opening index: {}", e);
            eprintln!("Hint: Run 'mdv reindex' to build the index first.");
            std::process::exit(1);
        }
    };

    // Find orphans
    let orphans = match db.find_orphans() {
        Ok(notes) => notes,
        Err(e) => {
            eprintln!("Error finding orphans: {}", e);
            std::process::exit(1);
        }
    };

    // Resolve output format
    let format = resolve_format(args.output, args.json, args.quiet);

    // Output results
    match format {
        OutputFormat::Table => print_notes_table(&orphans),
        OutputFormat::Json => print_notes_json(&orphans),
        OutputFormat::Quiet => print_notes_quiet(&orphans),
    }
}

fn resolve_format(output: OutputFormat, json: bool, quiet: bool) -> OutputFormat {
    if json {
        OutputFormat::Json
    } else if quiet {
        OutputFormat::Quiet
    } else {
        output
    }
}
