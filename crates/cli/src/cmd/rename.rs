//! Rename command implementation.

use std::io::{self, Write};
use std::path::Path;

use mdvault_core::config::loader::ConfigLoader;
use mdvault_core::index::IndexDb;
use mdvault_core::rename::{
    execute_rename, generate_preview, FileChange, RenameError, RenamePreview,
};

use crate::RenameArgs;

pub fn run(config: Option<&Path>, profile: Option<&str>, args: RenameArgs) {
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

    // Generate preview
    let preview = match generate_preview(&db, &rc.vault_root, &args.source, &args.dest) {
        Ok(p) => p,
        Err(e) => {
            print_error(&e);
            std::process::exit(1);
        }
    };

    // Display preview
    print_preview(&preview, &rc.vault_root);

    // If dry-run, stop here
    if args.dry_run {
        println!();
        println!("(dry-run mode - no changes made)");
        return;
    }

    // Confirm unless --yes
    if !args.yes && !confirm_rename() {
        println!("Cancelled.");
        return;
    }

    // Execute rename
    match execute_rename(&db, &rc.vault_root, &args.source, &args.dest) {
        Ok(result) => {
            println!();
            println!(
                "Renamed: {} -> {}",
                result
                    .old_path
                    .strip_prefix(&rc.vault_root)
                    .unwrap_or(&result.old_path)
                    .display(),
                result
                    .new_path
                    .strip_prefix(&rc.vault_root)
                    .unwrap_or(&result.new_path)
                    .display()
            );
            println!("Files modified: {}", result.files_modified.len());
            println!("References updated: {}", result.references_updated);

            // Print any warnings
            for warning in &result.warnings {
                eprintln!("{}", warning);
            }
        }
        Err(e) => {
            print_error(&e);
            std::process::exit(1);
        }
    }
}

fn print_error(e: &RenameError) {
    match e {
        RenameError::SourceNotFound(path) => {
            eprintln!("Error: Source file not found: {}", path.display());
        }
        RenameError::TargetExists(path) => {
            eprintln!("Error: Target file already exists: {}", path.display());
        }
        RenameError::NoteNotInIndex(path) => {
            eprintln!("Error: Note not found in index: {}", path.display());
            eprintln!("Hint: Run 'mdv reindex' to update the index.");
        }
        _ => {
            eprintln!("Error: {}", e);
        }
    }
}

fn print_preview(preview: &RenamePreview, vault_root: &Path) {
    let old_rel = preview.old_path.strip_prefix(vault_root).unwrap_or(&preview.old_path);
    let new_rel = preview.new_path.strip_prefix(vault_root).unwrap_or(&preview.new_path);

    println!("Renaming: {} -> {}", old_rel.display(), new_rel.display());
    println!();

    if preview.references.is_empty() {
        println!("No references found to update.");
    } else {
        println!(
            "Found {} reference(s) in {} file(s):",
            preview.total_references(),
            preview.files_affected()
        );
        println!();

        for change in &preview.changes {
            print_file_change(change, vault_root);
        }
    }

    // Print warnings
    for warning in &preview.warnings {
        println!();
        eprintln!("{}", warning);
    }
}

fn print_file_change(change: &FileChange, vault_root: &Path) {
    let rel_path = change.path.strip_prefix(vault_root).unwrap_or(&change.path);
    println!("{}:", rel_path.display());

    for reference in &change.references {
        let location = if reference.line_number > 0 {
            format!("  Line {}:", reference.line_number)
        } else {
            "  Frontmatter:".to_string()
        };

        // Show the original reference
        println!("{} {}", location, reference.original);
    }

    println!();
}

fn confirm_rename() -> bool {
    print!("Proceed? [y/N] ");
    io::stdout().flush().unwrap();

    let mut input = String::new();
    if io::stdin().read_line(&mut input).is_err() {
        return false;
    }

    let input = input.trim().to_lowercase();
    input == "y" || input == "yes"
}
