//! Links command implementation.

use std::path::Path;

use mdvault_core::config::loader::ConfigLoader;
use mdvault_core::index::IndexDb;

use super::output::{print_links_json, print_links_quiet, print_links_table, LinkOutput};
use crate::{LinksArgs, OutputFormat};

pub fn run(config: Option<&Path>, profile: Option<&str>, args: LinksArgs) {
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

    // Normalize the note path (strip leading ./)
    let note_path = normalize_path(&args.note);

    // Look up the note
    let note = match db.get_note_by_path(Path::new(&note_path)) {
        Ok(Some(n)) => n,
        Ok(None) => {
            eprintln!("Note not found in index: {}", note_path);
            eprintln!("Hint: Check the path or run 'mdv reindex'.");
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("Error looking up note: {}", e);
            std::process::exit(1);
        }
    };

    let note_id = note.id.expect("indexed note should have ID");

    // Determine what to show (both shown by default)
    let show_backlinks = args.backlinks || !args.outlinks;
    let show_outlinks = args.outlinks || !args.backlinks;

    // Resolve output format
    let format = resolve_format(args.output, args.json, args.quiet);

    // Get and display backlinks
    if show_backlinks {
        match db.get_backlinks(note_id) {
            Ok(links) => {
                let outputs: Vec<LinkOutput> = links
                    .iter()
                    .map(|l| {
                        // Look up source note path
                        let source_path = db
                            .get_note_by_id(l.source_id)
                            .ok()
                            .flatten()
                            .map(|n| n.path.to_string_lossy().to_string());
                        LinkOutput::from_link(l, source_path.as_deref())
                    })
                    .collect();

                if show_outlinks && !matches!(format, OutputFormat::Json) {
                    println!("=== Backlinks (notes linking to {}) ===", note_path);
                    println!();
                }
                match format {
                    OutputFormat::Table => print_links_table(&outputs, "backlinks"),
                    OutputFormat::Json => print_links_json(&outputs),
                    OutputFormat::Quiet => print_links_quiet(&outputs, true),
                }
            }
            Err(e) => {
                eprintln!("Error getting backlinks: {}", e);
                std::process::exit(1);
            }
        }
    }

    // Get and display outgoing links
    if show_outlinks {
        match db.get_outgoing_links(note_id) {
            Ok(links) => {
                let outputs: Vec<LinkOutput> = links
                    .iter()
                    .map(|l| LinkOutput::from_link(l, Some(&note_path)))
                    .collect();

                if show_backlinks && !matches!(format, OutputFormat::Json) {
                    println!();
                    println!("=== Outgoing links (notes {} links to) ===", note_path);
                    println!();
                }
                match format {
                    OutputFormat::Table => print_links_table(&outputs, "outgoing links"),
                    OutputFormat::Json => print_links_json(&outputs),
                    OutputFormat::Quiet => print_links_quiet(&outputs, false),
                }
            }
            Err(e) => {
                eprintln!("Error getting outgoing links: {}", e);
                std::process::exit(1);
            }
        }
    }
}

/// Normalize note path by removing leading ./.
fn normalize_path(path: &str) -> String {
    path.strip_prefix("./").unwrap_or(path).to_string()
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
